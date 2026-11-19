//! Pit design: benches projected from a closed string, with a haul ramp
//! carried through them.
//!
//! A pit is built bench by bench from its floor up, or from its crest down.
//! Each bench starts on a ring, rises (or falls) its height across a face at
//! the face angle, and leaves a berm before the next. Every ring is an
//! offset of the ring before it, never of the string the design started
//! from, so a ring follows whatever the benches below it did.
//!
//! The ramp crosses the benches on a slant. On each bench it leaves the ring
//! the bench starts from and has climbed the bench where it reaches the ring
//! the next one starts from, a face and a berm across, so the next bench's
//! stretch carries straight on (a spiral) or turns back on a flat pad (a
//! switchback). The berm under a stretch narrows to nothing where the road
//! takes its place; the wall above it stands back by the road's width. Where
//! the ramp begins, the wall behind it is benched back like any other wall.
//!
//! Places on a ring are found again on the next by where each point of the
//! next was offset from (`ring::Ring::sources`).

pub(crate) mod object;
pub(crate) mod ring;
pub(crate) mod road;
pub(crate) mod settings;
#[cfg(test)]
mod object_tests;
#[cfg(test)]
mod tests;

use glam::{DVec2, DVec3};

use self::{
    ring::EdgeOffset,
    road::{Lay, Stretch},
};

/// Which way a design is built from its string.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Direction {
    /// From a floor (toe) string, out and up.
    Up,
    /// From a crest string, in and down.
    Down,
}

impl Direction {
    /// Out from the ring (1) or in (-1), and up (1) or down (-1): the same.
    pub(crate) fn sign(self) -> f64 {
        match self {
            Self::Up => 1.0,
            Self::Down => -1.0,
        }
    }
}

/// How the ramp gets from one bench to the next.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Layout {
    /// On round the wall the way it was going.
    Spiral,
    /// Back the way it came, on a flat pad, every `every` benches; the road's
    /// inside edge turns no tighter than `inside_radius`.
    Switchback { every: u32, inside_radius: f64 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RampSpec {
    pub(crate) width: f64,
    /// Rise over run, held along its steeper edge.
    pub(crate) gradient: f64,
    /// Where it leaves the starting string: the point on it nearest this.
    pub(crate) start: DVec2,
    /// Whether it climbs clockwise seen from above.
    pub(crate) climbs_clockwise: bool,
    pub(crate) layout: Layout,
}

/// Where a design stops.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Stop {
    /// After this many benches.
    Benches(u32),
    /// At this RL, the last bench cut short to meet it.
    Elevation(f64),
    /// Built up until a whole bench stands clear of the ground.
    Daylight,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Spec {
    pub(crate) direction: Direction,
    pub(crate) bench_height: f64,
    /// Bench face angle from horizontal, in degrees.
    pub(crate) face_angle: f64,
    pub(crate) berm_width: f64,
    /// The tightest a wall may turn where the design would otherwise leave
    /// a sharp nose of rock (building up) or a sharp corner of pit (building
    /// down); also what keeps the ramp from turning tighter than its walls.
    pub(crate) wall_radius: f64,
    pub(crate) ramp: Option<RampSpec>,
    pub(crate) stop: Stop,
}

impl Spec {
    /// Why the settings cannot make a design, if they cannot.
    pub(crate) fn problem(&self) -> Option<Problem> {
        let positive = |value: f64| value.is_finite() && value > 0.0;
        if !positive(self.bench_height) {
            return Some(Problem::BenchHeight);
        }
        if !(self.face_angle.is_finite() && self.face_angle > 0.0 && self.face_angle < 90.0) {
            return Some(Problem::FaceAngle);
        }
        if !(self.berm_width.is_finite() && self.berm_width >= 0.0) {
            return Some(Problem::BermWidth);
        }
        if let Some(ramp) = self.ramp {
            if !positive(ramp.width) {
                return Some(Problem::RampWidth);
            }
            if !positive(ramp.gradient) {
                return Some(Problem::RampGradient);
            }
            if let Layout::Switchback { every, inside_radius } = ramp.layout
                && (every == 0 || !(inside_radius.is_finite() && inside_radius >= 0.0))
            {
                return Some(Problem::Switchback);
            }
        }
        match self.stop {
            Stop::Benches(0) => Some(Problem::Benches),
            Stop::Daylight if self.direction == Direction::Down => Some(Problem::DaylightDown),
            _ => None,
        }
    }

    /// The horizontal run of a face `height` high.
    fn face_run(&self, height: f64) -> f64 {
        height / self.face_angle.to_radians().tan()
    }
}

/// Why settings cannot make a design.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Problem {
    BenchHeight,
    FaceAngle,
    BermWidth,
    RampWidth,
    RampGradient,
    Switchback,
    Benches,
    /// Daylighting builds up to the ground; a design built down has none.
    DaylightDown,
    /// The stop elevation is behind the string, not ahead of it.
    StopElevation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum StringKind {
    Toe,
    Crest,
    RampEdge,
    RampCentreline,
}

impl StringKind {
    /// The colour a string is drawn in, by what it is.
    pub(crate) fn colour(self) -> [f32; 4] {
        match self {
            Self::Toe => [0.30, 0.60, 0.95, 1.0],
            Self::Crest => [0.90, 0.35, 0.25, 1.0],
            Self::RampEdge => [0.35, 0.80, 0.40, 1.0],
            Self::RampCentreline => [0.62, 0.66, 0.62, 1.0],
        }
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct DesignString {
    pub(crate) kind: StringKind,
    pub(crate) points: Vec<DVec3>,
    pub(crate) closed: bool,
}

/// What one bench came to, for checking against what was asked.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct BenchReport {
    /// The RL it starts from.
    pub(crate) elevation: f64,
    pub(crate) height: f64,
    /// The plan area inside the ring it ends on.
    pub(crate) area: f64,
    pub(crate) ramp: Option<RampReport>,
}

/// What a bench's stretch of ramp came to.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct RampReport {
    /// Its rise over its run along its steeper edge, from where it leaves to
    /// where it arrives.
    pub(crate) grade: f64,
    /// The steepest either edge gets over any metre of its run.
    pub(crate) steepest: f64,
    /// The narrowest the road is between its edges.
    pub(crate) narrowest: f64,
    /// How long a flat it runs level before it climbs, laid to leave the
    /// bench sound.
    pub(crate) flat: f64,
    /// How much longer than asked its climb is, where no flat would leave
    /// the bench sound: what makes it flatter than asked.
    pub(crate) lengthened: f64,
    /// Whether it turned back on a switchback to start.
    pub(crate) switchback: bool,
}

/// Why a design ended where it did.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum End {
    /// Every bench asked for was built.
    Complete,
    /// A whole bench stands clear of the ground.
    Daylight,
    /// The next bench had no room left in plan.
    OutOfRoom,
    /// The ring is too short to climb the next bench along at the gradient.
    RampTooLong,
    /// The design reached the edge of the ground before it daylit.
    LeftGround,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Design {
    pub(crate) strings: Vec<DesignString>,
    pub(crate) benches: u32,
    /// Each bench built, in the order built.
    pub(crate) report: Vec<BenchReport>,
    /// The RL each ring is at, the starting string's first.
    pub(crate) levels: Vec<f64>,
    pub(crate) end: End,
    /// Plan length of the ramp's centreline.
    pub(crate) ramp_length: f64,
    /// How far along the wall from where it was asked the ramp starts: off
    /// a tip too sharp for it, it starts where it fits.
    pub(crate) ramp_start_moved: f64,
    /// Where the ramp leaves the design: its centreline's last point
    /// building up, its first building down.
    pub(crate) ramp_exit: Option<DVec3>,
    /// The volume between the starting string and the last ring, from the
    /// rings' areas: the ground the benches enclose.
    pub(crate) volume: f64,
}

impl Design {
    /// The RL of the last ring built.
    pub(crate) fn final_elevation(&self) -> f64 {
        self.levels.last().copied().unwrap_or_default()
    }

    /// The same design moved by `delta`: a design moves with its string.
    pub(crate) fn translated(&self, delta: DVec3) -> Self {
        let mut moved = self.clone();
        for string in &mut moved.strings {
            for point in &mut string.points {
                *point += delta;
            }
        }
        for level in &mut moved.levels {
            *level += delta.z;
        }
        for bench in &mut moved.report {
            bench.elevation += delta.z;
        }
        moved.ramp_exit = moved.ramp_exit.map(|exit| exit + delta);
        moved
    }
}

/// What a closed starting string becomes before any bench is built.
#[derive(Clone, Debug)]
pub(crate) struct Base {
    ring: Vec<DVec2>,
    pub(crate) elevation: f64,
    /// How far the string's points stray from `elevation`.
    pub(crate) unevenness: f64,
}

impl Base {
    /// A closed string's plan outline and its level: its points' mean RL.
    pub(crate) fn from_points(points: &[DVec3]) -> Option<Self> {
        if points.len() < 3 {
            return None;
        }
        let elevation = points.iter().map(|point| point.z).sum::<f64>() / points.len() as f64;
        let unevenness = points.iter().map(|point| (point.z - elevation).abs()).fold(0.0, f64::max);
        let ring = ring::normalised(points.iter().map(|point| point.truncate()));
        (ring.len() >= 3 && ring::signed_area(&ring) > ring::SAME_POINT).then_some(Self { ring, elevation, unevenness })
    }
}

/// Where a ramp is on the ring the next bench starts from.
#[derive(Clone, Copy, Debug)]
struct RampCursor {
    at: f64,
    /// 1 with the ring's order, -1 against it, as the design is built.
    direction: f64,
    /// Where the stretch before arrived on its far edge, when the ramp
    /// carries on from one: the ring steps across the road from there to
    /// `at`, and that step is road, not wall.
    handover: Option<DVec2>,
    /// How many stretches the ramp has climbed since it began or last
    /// turned back.
    since_turn: u32,
    /// Which way in plan the road was heading as it arrived at `at`.
    heading: DVec2,
}

/// A bench's stretch of ramp as laid, with the switchback it turned back on
/// to start, if it did, and how far its start was moved to fit.
struct Laid {
    stretch: Stretch,
    turn: Option<Turn>,
    moved: f64,
    /// How long a flat it was laid with before its climb.
    flat: f64,
    /// How much longer than asked its climb was laid, where no flat would do.
    lengthened: f64,
}

/// How a bench's stretch of ramp joins what came before it.
#[derive(Clone, Copy)]
enum Join<'a> {
    /// The ramp begins on this bench.
    Begins,
    /// It carries on from the stretch before, which arrived across the road
    /// from `handover` to the parameter `at` of the ring.
    CarriesOn { handover: DVec2, at: f64 },
    /// It turns back on a switchback from the stretch before.
    TurnsBack(&'a Turn),
}

/// A switchback's turn, level, in plan: half circles from where the road
/// arrived to where it leaves the other way.
struct Turn {
    /// The inside of the turn, from where the arriving far edge ended to
    /// where the leaving near edge starts.
    inner: Vec<DVec2>,
    /// The outside, from where the arriving near edge ended to where the
    /// leaving far edge starts.
    outer: Vec<DVec2>,
    /// Its centreline.
    centre: Vec<DVec2>,
    /// The pad's edge that the bench's start ring goes round: from where the
    /// arriving far edge ended, across the road leaving, and back round the
    /// outside of the turn to where the arriving near edge ended.
    pad: Vec<DVec2>,
}

/// The ground's RL at a point, for daylighting.
pub(crate) type Ground<'a> = &'a dyn Fn(DVec2) -> Option<f64>;

/// Build the design a base string and its settings describe, stopping early
/// if `cancelled` says to.
pub(crate) fn design(base: &Base, spec: &Spec, ground: Option<Ground<'_>>, cancelled: &dyn Fn() -> bool) -> Result<Design, Problem> {
    let flats = design_with(base, spec, ground, cancelled, Additions::FlatsFirst)?;
    // A ramp kept to its gradient by flats can come to a bench with no room
    // where one climbing a little flatter would have found it, the benches
    // before having put it somewhere else: the design that gets further is
    // the one given, flats first where both get as far.
    if flats.end != End::OutOfRoom || spec.ramp.is_none() || cancelled() {
        return Ok(flats);
    }
    let longer = design_with(base, spec, ground, cancelled, Additions::LongerOnly)?;
    Ok(if longer.benches > flats.benches { longer } else { flats })
}

/// What a stretch may have added to it to leave its bench sound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Additions {
    /// A flat before its climb, keeping its gradient; failing every flat, a
    /// longer climb.
    FlatsFirst,
    /// Only a longer climb, a little flatter.
    LongerOnly,
}

fn design_with(base: &Base, spec: &Spec, ground: Option<Ground<'_>>, cancelled: &dyn Fn() -> bool, additions: Additions) -> Result<Design, Problem> {
    if let Some(problem) = spec.problem() {
        return Err(problem);
    }
    let sign = spec.direction.sign();
    if let Stop::Elevation(elevation) = spec.stop
        && (elevation - base.elevation) * sign <= 0.0
    {
        return Err(Problem::StopElevation);
    }
    let start = base.ring.clone();
    let mut state = State {
        builder: Builder {
            spec,
            sign,
            additions,
            strings: vec![level_string(spec, &start, base.elevation, true)],
            left: Vec::new(),
            right: Vec::new(),
            centreline: Vec::new(),
            ramp_length: 0.0,
            start_moved: 0.0,
            built: Built::default(),
        },
        ramp: spec.ramp.map(|ramp| RampCursor {
            at: ring::nearest(&start, ramp.start).0,
            // Building down walks the ramp from its top, the way it falls.
            direction: if ramp.climbs_clockwise == (spec.direction == Direction::Up) { -1.0 } else { 1.0 },
            handover: None,
            since_turn: 0,
            heading: DVec2::ZERO,
        }),
        current: start,
        elevation: base.elevation,
        levels: vec![base.elevation],
        benches: 0,
        volume: 0.0,
        report: Vec::new(),
    };
    // The state before each of the last few benches built, and what was
    // added to its stretch: where a bench cannot be built for want of room,
    // one of them is laid again with more added, and the benches after it
    // built again from there.
    let mut history: History<'_> = Vec::new();
    let end = loop {
        if cancelled() {
            break End::Complete;
        }
        let Some(height) = state.next_height() else {
            break End::Complete;
        };
        let snapshot = state.clone();
        match state.builder.bench(&state.current, state.elevation, height, state.berm(), state.ramp, None) {
            Ok(bench) => {
                let added = bench.added;
                state.advance(bench, height);
                remember(&mut history, snapshot, added);
            }
            Err(End::OutOfRoom) if !cancelled() => match (1..=history.len()).find_map(|depth| relaid(&history, depth, state.benches)) {
                Some((relaid, rebuilt)) => {
                    state = relaid;
                    history = rebuilt;
                }
                None => break End::OutOfRoom,
            },
            Err(end) => break end,
        }
        if spec.stop == Stop::Daylight
            && let Some(ground) = ground
        {
            match clear_of_ground(&state.current, state.elevation, ground) {
                Some(true) => break End::Daylight,
                Some(false) => {}
                None => break End::LeftGround,
            }
        }
    };
    let State {
        builder,
        levels,
        benches,
        volume,
        report,
        ..
    } = state;
    let ramp_exit = match spec.direction {
        Direction::Up => builder.centreline.last().copied(),
        Direction::Down => builder.centreline.first().copied(),
    };
    let Builder {
        mut strings,
        left,
        right,
        centreline,
        ramp_length,
        start_moved,
        ..
    } = builder;
    for (points, kind) in [(left, StringKind::RampEdge), (right, StringKind::RampEdge), (centreline, StringKind::RampCentreline)] {
        if points.len() >= 2 {
            strings.push(DesignString { kind, points, closed: false });
        }
    }
    Ok(Design {
        strings,
        benches,
        report,
        levels,
        end,
        ramp_length,
        ramp_start_moved: start_moved,
        ramp_exit,
        volume,
    })
}

/// A design as far as it has been built.
#[derive(Clone)]
struct State<'a> {
    builder: Builder<'a>,
    /// The ring the next bench starts from, at `elevation`.
    current: Vec<DVec2>,
    elevation: f64,
    levels: Vec<f64>,
    ramp: Option<RampCursor>,
    benches: u32,
    volume: f64,
    report: Vec<BenchReport>,
}

impl State<'_> {
    /// How high the next bench is, or `None` if the design is complete.
    fn next_height(&self) -> Option<f64> {
        let spec = self.builder.spec;
        match spec.stop {
            Stop::Benches(count) if self.benches >= count => None,
            Stop::Elevation(stop) => {
                let left = (stop - self.elevation) * self.builder.sign;
                (left > 1.0e-6).then(|| left.min(spec.bench_height))
            }
            _ => Some(spec.bench_height),
        }
    }

    /// The berm the next bench leaves first: none before the first.
    fn berm(&self) -> f64 {
        if self.benches == 0 { 0.0 } else { self.builder.spec.berm_width }
    }

    /// Take a bench built `height` high from where the design stands.
    fn advance(&mut self, bench: Bench, height: f64) {
        self.volume += frustum(ring::signed_area(&self.current), ring::signed_area(&bench.end), height);
        self.report.push(BenchReport {
            elevation: self.elevation,
            height,
            area: ring::signed_area(&bench.end),
            ramp: bench.ramp,
        });
        self.elevation += self.builder.sign * height;
        self.levels.push(self.elevation);
        self.benches += 1;
        self.ramp = bench.next_ramp;
        self.current = bench.end;
    }
}

/// How many benches back a design goes to lay one again, where a bench
/// cannot be built for want of room.
const RELAY_DEPTH: usize = 3;

/// How many times a bench is laid again, each time with more added to its
/// stretch, before going back a bench further.
const RELAYS: usize = 4;

/// The states before the last few benches built, each with what was added
/// to its bench's stretch: what a design can go back to.
type History<'a> = Vec<(State<'a>, Option<Added>)>;

/// Keep the state before a bench built, and what was added to its stretch,
/// as one of the last few a design can go back to.
fn remember<'a>(history: &mut History<'a>, before: State<'a>, added: Option<Added>) {
    history.push((before, added));
    if history.len() > RELAY_DEPTH {
        history.remove(0);
    }
}

/// The design with the bench `depth` back in `history` laid again, each
/// time with more added to its stretch than it had, and the benches after it
/// built again, until the design holds `benches` and one more: the design,
/// and the history to go back through from there. `None` if no relaying
/// lets them all be built.
fn relaid<'a>(history: &[(State<'a>, Option<Added>)], depth: usize, benches: u32) -> Option<(State<'a>, History<'a>)> {
    let (before, added) = history.get(history.len().checked_sub(depth)?)?;
    let mut after = (*added)?;
    for _ in 0..RELAYS {
        let mut trial = before.clone();
        let mut rebuilt = history[..history.len() - depth].to_vec();
        let height = trial.next_height()?;
        let snapshot = trial.clone();
        let bench = trial.builder.bench(&trial.current, trial.elevation, height, trial.berm(), trial.ramp, Some(after)).ok()?;
        after = bench.added?;
        trial.advance(bench, height);
        remember(&mut rebuilt, snapshot, Some(after));
        let built = loop {
            if trial.benches > benches {
                break true;
            }
            let Some(height) = trial.next_height() else {
                break true;
            };
            let snapshot = trial.clone();
            match trial.builder.bench(&trial.current, trial.elevation, height, trial.berm(), trial.ramp, None) {
                Ok(bench) => {
                    let added = bench.added;
                    trial.advance(bench, height);
                    remember(&mut rebuilt, snapshot, added);
                }
                Err(_) => break false,
            }
        };
        if built {
            return Some((trial, rebuilt));
        }
    }
    None
}

/// Whether a whole ring stands above the ground: `None` where it leaves the
/// ground's edge.
fn clear_of_ground(points: &[DVec2], elevation: f64, ground: Ground<'_>) -> Option<bool> {
    let arcs = ring::arcs(points);
    let around = arcs[points.len()];
    let samples = ((around / 2.0).ceil() as usize).max(points.len());
    for sample in 0..samples {
        let point = ring::point_at(points, ring::parameter_at(&arcs, around * sample as f64 / samples as f64));
        if ground(point)? > elevation {
            return Some(false);
        }
    }
    Some(true)
}

#[derive(Clone)]
struct Builder<'a> {
    spec: &'a Spec,
    sign: f64,
    additions: Additions,
    strings: Vec<DesignString>,
    /// The ramp's edges, left and right of the way it is built, and its
    /// centreline, joined up bench to bench.
    left: Vec<DVec3>,
    right: Vec<DVec3>,
    centreline: Vec<DVec3>,
    ramp_length: f64,
    /// How far along the wall from where it was asked to the ramp had to
    /// start to fit.
    start_moved: f64,
    /// Every string built so far, for keeping the next bench clear of them.
    built: Built,
}

/// Strings already built, as segments binned in a grid over the plan.
#[derive(Clone, Default)]
struct Built {
    segments: Vec<(DVec3, DVec3)>,
    cells: std::collections::HashMap<(i64, i64), Vec<u32>>,
}

/// The size of a cell of the grid built strings are binned in.
const BUILT_CELL: f64 = 25.0;

/// Strings this far apart in height where they cross in plan are at the
/// same height: a road meeting the ring it arrives on, say.
const SAME_HEIGHT: f64 = 0.05;

impl Built {
    fn cells_of(a: DVec3, b: DVec3) -> impl Iterator<Item = (i64, i64)> {
        let cell = |value: f64| (value / BUILT_CELL).floor() as i64;
        let (x0, x1) = (cell(a.x.min(b.x)), cell(a.x.max(b.x)));
        let (y0, y1) = (cell(a.y.min(b.y)), cell(a.y.max(b.y)));
        (x0..=x1).flat_map(move |x| (y0..=y1).map(move |y| (x, y)))
    }

    fn add(&mut self, segments: impl Iterator<Item = (DVec3, DVec3)>) {
        for (a, b) in segments {
            let index = self.segments.len() as u32;
            self.segments.push((a, b));
            for cell in Self::cells_of(a, b) {
                self.cells.entry(cell).or_default().push(index);
            }
        }
    }

    /// Whether any of `candidate` crosses a built string in plan at a
    /// different height.
    fn crossed_by(&self, candidate: &[(DVec3, DVec3)]) -> bool {
        let mut seen = std::collections::HashSet::new();
        candidate.iter().any(|(a, b)| {
            seen.clear();
            Self::cells_of(*a, *b).any(|cell| {
                self.cells.get(&cell).is_some_and(|indices| {
                    indices.iter().any(|&index| {
                        if !seen.insert(index) {
                            return false;
                        }
                        let (c, d) = self.segments[index as usize];
                        match crate::model::kernel::segment_segment(a.truncate(), b.truncate(), c.truncate(), d.truncate()) {
                            crate::model::kernel::SegSeg::Crossing { t, u, .. } => (a.z + (b.z - a.z) * t - (c.z + (d.z - c.z) * u)).abs() > SAME_HEIGHT,
                            _ => false,
                        }
                    })
                })
            })
        })
    }
}

/// One bench built: the ring the next starts from, where its ramp is, and
/// what its stretch of ramp came to.
struct Bench {
    end: Vec<DVec2>,
    next_ramp: Option<RampCursor>,
    ramp: Option<RampReport>,
    /// What was added to its stretch to leave it sound, if it has one.
    added: Option<Added>,
}

/// Length added to a stretch to leave its bench sound: a flat before its
/// climb, or failing every flat, a longer climb. Ordered from least to
/// most: every flat before any longer climb.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Added {
    longer: bool,
    length: f64,
}

/// How far a ramp's start is moved at a time to find where it fits.
const START_STEP: f64 = 2.0;

/// Points this close are where a road was meant to meet a ring.
const MEETS: f64 = 1.0e-3;

impl Builder<'_> {
    /// Build one bench `height` high from the ring `from` at `elevation`,
    /// after a berm `berm` wide.
    ///
    /// Where the ramp's stretch would leave the bench unsound (its handover
    /// crowding a corner, say), the stretch is laid a little longer, a hair
    /// flatter and never steeper, until it does not.
    ///
    /// With `after`, only additions after it are tried: the bench is being
    /// laid again because the one after it would not build.
    fn bench(&mut self, from: &[DVec2], elevation: f64, height: f64, berm: f64, ramp: Option<RampCursor>, after: Option<Added>) -> Result<Bench, End> {
        let spec = self.spec;
        let sign = self.sign;
        let face = spec.face_run(height);
        let (laid, start_ring, end_ring) = match (ramp, spec.ramp) {
            (Some(cursor), Some(ramp_spec)) => {
                // Time for a switchback: tried first, and where it will not
                // fit, the ramp carries on round for another bench and
                // tries again on the next.
                let turn = match ramp_spec.layout {
                    Layout::Switchback { every, inside_radius } if cursor.handover.is_some() && cursor.since_turn >= every => Some(inside_radius),
                    _ => None,
                };
                let mut last = End::OutOfRoom;
                let mut found = None;
                for turn in [turn, None].into_iter().take(if turn.is_some() { 2 } else { 1 }) {
                    // A flat before the climb first, which keeps the grade;
                    // only where none works, a longer climb, a little flatter.
                    let climb = height / ramp_spec.gradient;
                    let lay = |flat: f64, longer: f64| Lay {
                        side: sign,
                        drift: berm + face,
                        width: ramp_spec.width,
                        climb_length: climb + longer,
                        flat,
                    };
                    let flats_after = match (self.additions, after) {
                        (Additions::LongerOnly, _) | (_, Some(Added { longer: true, .. })) => None,
                        (Additions::FlatsFirst, None) => Some(None),
                        (Additions::FlatsFirst, Some(Added { longer: false, length })) => Some(Some(length)),
                    };
                    let longer_after = match after {
                        Some(Added { longer: true, length }) => Some(length),
                        _ => None,
                    };
                    let flat = || match flats_after {
                        Some(floor) => shortest(floor, &mut |flat| self.attempt(from, cursor, lay(flat, 0.0), turn, berm, face, elevation, height)),
                        None => Err(End::OutOfRoom),
                    };
                    let tried = flat().or_else(|_| shortest(longer_after, &mut |longer| self.attempt(from, cursor, lay(0.0, longer), turn, berm, face, elevation, height)));
                    match tried {
                        Ok(attempt) => {
                            found = Some(attempt);
                            break;
                        }
                        Err(end) => last = end,
                    }
                }
                let (laid, start_ring, end_ring) = found.ok_or(last)?;
                self.start_moved = self.start_moved.max(laid.moved);
                (Some(laid), start_ring, end_ring)
            }
            _ => {
                let (start_ring, end_ring) = rings(spec, from, berm, face, None).and_then(|rings| self.clear_of_built(rings, None, None, elevation, height))?;
                (None, start_ring, end_ring)
            }
        };

        let end_elevation = elevation + sign * height;
        // With no berm the bench starts on the ring the last one ended on,
        // which is a string already.
        if berm > 0.0 {
            self.strings.push(level_string(spec, &start_ring, elevation, true));
        }
        self.strings.push(level_string(spec, &end_ring, end_elevation, false));
        for (ring, z) in [(&start_ring, elevation), (&end_ring, end_elevation)] {
            self.built
                .add((0..ring.len()).map(|index| (ring[index].extend(z), ring[(index + 1) % ring.len()].extend(z))));
        }

        // Where the ramp carries on from on the ring the next bench starts
        // from: where its near edge arrived.
        let mut ramp_report = None;
        let next_ramp = match (&laid, ramp) {
            (Some(laid), Some(cursor)) => {
                let (near, far) = self.lay_road(&laid.stretch, laid.turn.as_ref(), from, elevation, height);
                ramp_report = Some(RampReport {
                    flat: laid.flat,
                    lengthened: laid.lengthened,
                    switchback: laid.turn.is_some(),
                    ..measure_road(&near, &far, height)
                });
                let (Some(near_end), Some(far_end)) = (laid.stretch.edges.near.last(), laid.stretch.edges.far.last()) else {
                    return Err(End::OutOfRoom);
                };
                let edge = &laid.stretch.edges.near;
                let heading = edge
                    .len()
                    .checked_sub(2)
                    .and_then(|before| (near_end.0 - edge[before].0).try_normalize())
                    .unwrap_or(DVec2::ZERO);
                Some(RampCursor {
                    at: ring::nearest(&end_ring, near_end.0).0,
                    direction: laid.stretch.direction,
                    handover: Some(far_end.0),
                    since_turn: if laid.turn.is_some() { 1 } else { cursor.since_turn + 1 },
                    heading,
                })
            }
            _ => None,
        };
        let added = laid.as_ref().map(|laid| Added {
            longer: laid.lengthened > 0.0,
            length: if laid.lengthened > 0.0 { laid.lengthened } else { laid.flat },
        });
        Ok(Bench {
            end: end_ring,
            next_ramp,
            ramp: ramp_report,
            added,
        })
    }

    /// One try at a bench's stretch of ramp and its rings: carried on from
    /// the last (or begun), or turned back on a pad `turn` the inside radius
    /// of its turn.
    #[allow(clippy::too_many_arguments)]
    fn attempt(
        &self,
        from: &[DVec2],
        cursor: RampCursor,
        lay: Lay,
        turn: Option<f64>,
        berm: f64,
        face: f64,
        elevation: f64,
        height: f64,
    ) -> Result<(Laid, Vec<DVec2>, Vec<DVec2>), End> {
        let (stretch, turn, moved) = match (turn, cursor.handover) {
            (Some(radius), Some(handover)) => {
                let (stretch, turn) = turn_back(from, cursor, handover, lay, radius)?;
                (stretch, Some(turn), 0.0)
            }
            _ => {
                let (stretch, moved) = lay_stretch(from, cursor, lay)?;
                (stretch, None, moved)
            }
        };
        let join = match (&turn, cursor.handover) {
            (Some(turn), _) => Join::TurnsBack(turn),
            (None, Some(handover)) => Join::CarriesOn { handover, at: cursor.at },
            (None, None) => Join::Begins,
        };
        let (start_ring, end_ring) = rings(self.spec, from, berm, face, Some((&stretch, join)))?;
        let (start_ring, end_ring) = self.clear_of_built((start_ring, end_ring), Some(&stretch), turn.as_ref(), elevation, height)?;
        Ok((
            Laid {
                stretch,
                turn,
                moved,
                flat: lay.flat,
                lengthened: lay.climb_length - height / self.spec.ramp.map_or(1.0, |ramp| ramp.gradient),
            },
            start_ring,
            end_ring,
        ))
    }

    /// A bench's rings, if they and its stretch keep clear of everything
    /// built before them: no string of the design may cross another at a
    /// different height, so a bench that would is not built.
    fn clear_of_built(
        &self,
        rings: (Vec<DVec2>, Vec<DVec2>),
        stretch: Option<&Stretch>,
        turn: Option<&Turn>,
        elevation: f64,
        height: f64,
    ) -> Result<(Vec<DVec2>, Vec<DVec2>), End> {
        let end_elevation = elevation + self.sign * height;
        let mut candidate: Vec<(DVec3, DVec3)> = Vec::new();
        for (ring, z) in [(&rings.0, elevation), (&rings.1, end_elevation)] {
            candidate.extend((0..ring.len()).map(|index| (ring[index].extend(z), ring[(index + 1) % ring.len()].extend(z))));
        }
        if let Some(stretch) = stretch {
            for edge in [&stretch.edges.near, &stretch.edges.far] {
                let points: Vec<DVec3> = edge
                    .iter()
                    .map(|(point, source)| point.extend(elevation + self.sign * height * stretch.climbed_at(*source).unwrap_or(1.0)))
                    .collect();
                candidate.extend(points.windows(2).map(|pair| (pair[0], pair[1])));
            }
        }
        if let Some(turn) = turn {
            for edge in [&turn.inner, &turn.outer] {
                candidate.extend(edge.windows(2).map(|pair| (pair[0].extend(elevation), pair[1].extend(elevation))));
            }
        }
        if self.built.crossed_by(&candidate) { Err(End::OutOfRoom) } else { Ok(rings) }
    }

    /// Add a stretch's edges and centreline to the ramp's strings, and give
    /// back its edges as laid, near then far.
    fn lay_road(&mut self, stretch: &Stretch, turn: Option<&Turn>, from: &[DVec2], elevation: f64, height: f64) -> (Vec<DVec3>, Vec<DVec3>) {
        if let Some(turn) = turn {
            // The pad, level: each edge of the road arriving runs on round
            // the turn, the near one the outside of it.
            let level = |points: &[DVec2]| -> Vec<DVec3> { points.iter().map(|point| point.extend(elevation)).collect() };
            let (outer, inner) = (level(&turn.outer), level(&turn.inner));
            let near_on_left = self.left.last().zip(outer.first()).is_some_and(|(left, start)| left.distance(*start) <= HANDOVER_GAP);
            let (left, right) = if near_on_left { (outer, inner) } else { (inner, outer) };
            for edge in [&left, &right] {
                self.built.add(edge.windows(2).map(|pair| (pair[0], pair[1])));
            }
            join(&mut self.left, left);
            join(&mut self.right, right);
            let centre = level(&turn.centre);
            self.ramp_length += turn.centre.windows(2).map(|pair| pair[0].distance(pair[1])).sum::<f64>();
            join(&mut self.centreline, centre);
        }
        let rise = self.sign * height;
        let at_height = |chain: &[(DVec2, f64)]| -> Vec<DVec3> {
            chain
                .iter()
                .map(|(point, source)| DVec3::new(point.x, point.y, elevation + rise * stretch.climbed_at(*source).unwrap_or(1.0)))
                .collect()
        };
        let (near, far) = (at_height(&stretch.edges.near), at_height(&stretch.edges.far));
        for edge in [&near, &far] {
            self.built.add(edge.windows(2).map(|pair| (pair[0], pair[1])));
        }
        // Building out from an anticlockwise ring, the far edge is on the
        // left of a road running against the ring's order.
        let far_on_left = self.sign * stretch.direction < 0.0;
        let (left, right) = if far_on_left { (far.clone(), near.clone()) } else { (near.clone(), far.clone()) };
        join(&mut self.left, left);
        join(&mut self.right, right);
        // The centreline at each station, half way between the edges.
        let centre: Vec<DVec3> = stretch
            .stations
            .iter()
            .map(|(parameter, climbed)| {
                let near = point_by_source(&stretch.edges.near, *parameter, from.len());
                let far = point_by_source(&stretch.edges.far, *parameter, from.len());
                let middle = near.lerp(far, 0.5);
                DVec3::new(middle.x, middle.y, elevation + rise * climbed)
            })
            .collect();
        self.ramp_length += centre.windows(2).map(|pair| pair[0].truncate().distance(pair[1].truncate())).sum::<f64>();
        join(&mut self.centreline, centre);
        (near, far)
    }
}

/// What a stretch's edges, as laid, come to: its grade along the steeper
/// edge over its climb (its flat left out), the steepest metre of either,
/// and the narrowest the road gets.
fn measure_road(near: &[DVec3], far: &[DVec3], height: f64) -> RampReport {
    let run = |edge: &[DVec3]| {
        edge.windows(2)
            .filter(|pair| (pair[1].z - pair[0].z).abs() > 1.0e-9)
            .map(|pair| pair[0].truncate().distance(pair[1].truncate()))
            .sum::<f64>()
    };
    let shorter = run(near).min(run(far));
    let grade = if shorter > 0.0 { height / shorter } else { 0.0 };
    let mut steepest: f64 = 0.0;
    for edge in [near, far] {
        let Some(&first) = edge.first() else {
            continue;
        };
        let (mut anchor, mut run) = (first, 0.0);
        for pair in edge.windows(2) {
            run += pair[0].truncate().distance(pair[1].truncate());
            if run >= 1.0 {
                steepest = steepest.max((pair[1].z - anchor.z).abs() / run);
                anchor = pair[1];
                run = 0.0;
            }
        }
    }
    let mut narrowest = f64::INFINITY;
    for point in near {
        let nearest = far
            .windows(2)
            .filter(|pair| point.z >= pair[0].z.min(pair[1].z) - SAME_HEIGHT && point.z <= pair[0].z.max(pair[1].z) + SAME_HEIGHT)
            .map(|pair| crate::model::kernel::project_onto_segment(point.truncate(), pair[0].truncate(), pair[1].truncate()).0.distance(point.truncate()))
            .fold(f64::INFINITY, f64::min);
        narrowest = narrowest.min(nearest);
    }
    RampReport {
        grade,
        steepest,
        narrowest: if narrowest.is_finite() { narrowest } else { 0.0 },
        flat: 0.0,
        lengthened: 0.0,
        switchback: false,
    }
}

/// The lengths added to a stretch, in turn, where it would leave its bench
/// unsound as laid (a flat before its climb, or failing that a longer
/// climb): the first that works is then shortened as far as it will go.
const ADDED: [f64; 7] = [0.0, 5.0, 10.0, 20.0, 30.0, 45.0, 60.0];

/// How close the shortest length added that works is found.
const ADDED_TOLERANCE: f64 = 0.5;

/// A stretch tried with the shortest length added that leaves the bench
/// sound: none if it can be, else the first of [`ADDED`] that works,
/// shortened by halves toward the one before it that did not. With
/// `floor`, only lengths more than it are tried, by at least the tolerance.
fn shortest<T>(floor: Option<f64>, attempt: &mut impl FnMut(f64) -> Result<T, End>) -> Result<T, End> {
    // Why the first try failed: the longer ones are only less likely to fit.
    let mut first: Option<End> = None;
    let mut below = floor;
    let least = floor.map_or(f64::NEG_INFINITY, |floor| floor + ADDED_TOLERANCE);
    for added in ADDED.into_iter().filter(|added| *added >= least) {
        match attempt(added) {
            Ok(found) => {
                let Some(mut low) = below else {
                    return Ok(found);
                };
                let (mut high, mut best) = (added, found);
                while high - low > ADDED_TOLERANCE {
                    let middle = (low + high) * 0.5;
                    match attempt(middle) {
                        Ok(found) => {
                            high = middle;
                            best = found;
                        }
                        Err(_) => low = middle,
                    }
                }
                return Ok(best);
            }
            Err(end) => {
                first.get_or_insert(end);
                below = Some(added);
            }
        }
    }
    Err(first.unwrap_or(End::OutOfRoom))
}

/// Lay a bench's stretch of ramp from `cursor` along `from`, and how far
/// along the wall it had to start from where it was asked to fit: a ramp
/// that begins off a tip too sharp to stand its width beside starts a
/// little further along.
fn lay_stretch(from: &[DVec2], cursor: RampCursor, lay: Lay) -> Result<(Stretch, f64), End> {
    if let Some(handover) = cursor.handover {
        let mut stretch = road::lay(from, cursor.at, cursor.direction, lay, &|_| 0.0).map_err(|_| End::RampTooLong)?;
        // It starts exactly where the last one arrived: laid off the ring's
        // corner there, its far edge can start a little to one side.
        if let Some(first) = stretch.edges.far.first_mut()
            && first.0.distance(handover) <= HANDOVER_GAP
        {
            first.0 = handover;
        }
        return Ok((stretch, 0.0));
    }
    let arcs = ring::arcs(from);
    let around = arcs[from.len()];
    let picked = ring::arc_at(&arcs, cursor.at);
    let mut moved = 0.0;
    loop {
        let at = ring::parameter_at(&arcs, picked + cursor.direction * moved);
        let stretch = road::lay(from, at, cursor.direction, lay, &|_| 0.0).map_err(|_| End::RampTooLong)?;
        if fits_against(from, &stretch) {
            return Ok((stretch, moved));
        }
        moved += START_STEP;
        if moved > around * 0.5 {
            return Err(End::OutOfRoom);
        }
    }
}

/// Turn the ramp back on a switchback where the stretch before arrived,
/// across the road from `handover` to the parameter `cursor.at` of `from`.
///
/// The road turns outward (building up; inward building down) through half
/// a circle, its inside edge no tighter than `radius`, on a pad level with
/// the ring, and leaves back the way it came on a stretch laid along the
/// same ring the other way. That stretch starts the turn's width out from
/// the ring, clear of the road below it, and comes in to its usual line by
/// the time it arrives: the island between the two roads narrows to
/// nothing.
fn turn_back(from: &[DVec2], cursor: RampCursor, handover: DVec2, lay: Lay, radius: f64) -> Result<(Stretch, Turn), End> {
    let near_end = ring::point_at(from, cursor.at);
    let across = (handover - near_end).try_normalize().ok_or(End::OutOfRoom)?;
    // Which way the road was heading as it arrived, square to the step
    // across it.
    let along = (cursor.heading - across * cursor.heading.dot(across)).try_normalize().ok_or(End::OutOfRoom)?;
    let reach = 2.0 * radius;
    let back_at = ring::nearest(from, handover).0;
    let stretch = road::lay(from, back_at, -cursor.direction, lay, &|climbed| reach * (1.0 - climbed)).map_err(|_| End::RampTooLong)?;
    let (Some(leave_near), Some(leave_far)) = (stretch.edges.near.first(), stretch.edges.far.first()) else {
        return Err(End::OutOfRoom);
    };
    let centre = handover + across * radius;
    let inner = half_turn(handover, leave_near.0, centre, along);
    let outer = half_turn(near_end, leave_far.0, centre, along);
    let middle = half_turn(near_end.lerp(handover, 0.5), leave_near.0.lerp(leave_far.0, 0.5), centre, along);
    let mut pad = vec![handover, leave_near.0];
    pad.extend(outer.iter().rev().copied());
    pad.dedup_by(|a, b| a.distance(*b) <= ring::SAME_POINT);
    Ok((
        stretch,
        Turn {
            inner,
            outer,
            centre: middle,
            pad,
        },
    ))
}

/// Half a turn round `centre` from `from` to `to`, bulging the way `ahead`
/// points: both ends included, and between them points whose chords touch
/// the circle (its radius going evenly from one end's to the other's), so
/// the road is nowhere narrower round it than at its ends.
fn half_turn(from: DVec2, to: DVec2, centre: DVec2, ahead: DVec2) -> Vec<DVec2> {
    let (start, end) = (from - centre, to - centre);
    let (start_radius, end_radius) = (start.length(), end.length());
    let (start_angle, end_angle) = (start.to_angle(), end.to_angle());
    let anticlockwise = (end_angle - start_angle).rem_euclid(std::f64::consts::TAU);
    let sweep = if DVec2::from_angle(start_angle + anticlockwise * 0.5).dot(ahead) >= 0.0 {
        anticlockwise
    } else {
        anticlockwise - std::f64::consts::TAU
    };
    let steps = ((sweep.abs() / ring::ARC_STEP).ceil() as usize).max(1);
    let half_step = sweep.abs() / steps as f64 * 0.5;
    let mut points = vec![from];
    for step in 1..steps {
        let t = step as f64 / steps as f64;
        let radius = (start_radius + (end_radius - start_radius) * t) / half_step.cos();
        points.push(centre + DVec2::from_angle(start_angle + sweep * t) * radius);
    }
    points.push(to);
    points
}

/// A bench's start ring with a switchback's pad joined to it: the ring
/// carried round `pad`'s edge (an open path from on the ring, out round the
/// pad and back across it) instead of through the pad. Of the two ways to
/// join them, the one that keeps most of the ring is the ring round the pad:
/// the other is only the bit the pad covers.
fn with_pad(start_ring: &[DVec2], pad: &[DVec2]) -> Option<Vec<DVec2>> {
    let count = start_ring.len();
    let &first = pad.first()?;
    let (at, gap) = start_ring
        .iter()
        .enumerate()
        .map(|(index, point)| (index, point.distance(first)))
        .min_by(|a, b| a.1.total_cmp(&b.1))?;
    if gap > HANDOVER_GAP || count < 3 {
        return None;
    }
    // Where the pad's edge, from the ring, first comes back to it.
    let mut met = None;
    'walk: for (step, pair) in pad.windows(2).enumerate() {
        let mut nearest: Option<(f64, usize, DVec2)> = None;
        for edge in 0..count {
            // Touching counts: with no berm the pad comes back to the ring
            // exactly where the road arrived.
            if let crate::model::kernel::SegSeg::Crossing { t, point, .. } | crate::model::kernel::SegSeg::Touching { t, point, .. } =
                crate::model::kernel::segment_segment(pair[0], pair[1], start_ring[edge], start_ring[(edge + 1) % count])
                && (step > 0 || t > 1.0e-6)
                && nearest.is_none_or(|(best, _, _)| t < best)
            {
                nearest = Some((t, edge, point));
            }
        }
        if let Some((_, edge, point)) = nearest {
            met = Some((step, edge, point));
            break 'walk;
        }
    }
    let (step, edge, point) = met?;
    let path: Vec<DVec2> = pad[..=step].iter().copied().chain(std::iter::once(point)).collect();
    let mut forward = Vec::with_capacity(count + path.len());
    let mut k = (edge + 1) % count;
    while k != at {
        forward.push(start_ring[k]);
        k = (k + 1) % count;
    }
    forward.extend(path.iter().copied());
    let mut backward = Vec::with_capacity(count + path.len());
    let mut k = (at + 1) % count;
    while k != (edge + 1) % count {
        backward.push(start_ring[k]);
        k = (k + 1) % count;
    }
    backward.extend(path.iter().rev().copied());
    let simple = |ring: &Vec<DVec2>| {
        let segments: Vec<(DVec2, DVec2)> = (0..ring.len()).map(|index| (ring[index], ring[(index + 1) % ring.len()])).collect();
        ring.len() >= 3 && ring::signed_area(ring) > 0.0 && !crosses(&segments, &segments)
    };
    [ring::cleaned(forward), ring::cleaned(backward)]
        .into_iter()
        .filter(simple)
        .max_by(|a, b| ring::signed_area(a).total_cmp(&ring::signed_area(b)))
}

/// The two rings of a bench from `from`: the one it starts on, across the
/// berm, and the one it ends on, across the face; or why there are none
/// that make a sound bench.
///
/// The berm narrows to nothing under the stretch, where the road takes its
/// place; where the stretch carries on from the one before, the ring steps
/// across the road to it, and that step is road, not set off at all. Beside
/// the stretch the face rises from the road's far edge, and is shorter the
/// higher the road has climbed, meeting it where it arrives.
fn rings(spec: &Spec, from: &[DVec2], berm: f64, face: f64, stretch: Option<(&Stretch, Join<'_>)>) -> Result<(Vec<DVec2>, Vec<DVec2>), End> {
    let sign = spec.direction.sign();
    let count = from.len();
    let start_ring = if berm == 0.0 {
        from.to_vec()
    } else {
        // A point at each of the stretch's stations, where its edges have
        // theirs: the berm narrows under the road point for point with it,
        // and so stays the road's own side of its near edge.
        let cuts: Vec<f64> = stretch.map_or_else(Vec::new, |(stretch, _)| stretch.stations.iter().map(|(parameter, _)| *parameter).collect());
        let (split, at) = ring::with_points_at(from, &cuts);
        let handover = stretch.and_then(|(stretch, join)| match join {
            Join::CarriesOn { handover, .. } => Some((handover, ring::point_at(from, stretch.from))),
            _ => None,
        });
        let distances: Vec<EdgeOffset> = (0..split.len())
            .map(|index| {
                let next = (index + 1) % split.len();
                let is = |a: DVec2, b: DVec2| {
                    (split[index].distance(a) <= MEETS && split[next].distance(b) <= MEETS) || (split[index].distance(b) <= MEETS && split[next].distance(a) <= MEETS)
                };
                if handover.is_some_and(|(pushed, start)| is(pushed, start)) {
                    return EdgeOffset::even(0.0);
                }
                let middle = middle_parameter(at[index], at[next], count);
                match stretch.filter(|(stretch, _)| stretch.climbed_at(middle).is_some()) {
                    Some((stretch, _)) => EdgeOffset {
                        start: sign * berm * stretch.climbed_at(at[index]).unwrap_or(0.0),
                        end: sign * berm * stretch.climbed_at(at[next]).unwrap_or(1.0),
                    },
                    None => EdgeOffset::even(sign * berm),
                }
            })
            .collect();
        let start_ring = ring::offset(&split, &distances).into_iter().next().ok_or(End::OutOfRoom)?;
        match stretch {
            Some((stretch, Join::CarriesOn { .. })) => along_far_edge(start_ring, stretch),
            _ => start_ring,
        }
    };
    // A switchback's pad is level with the bench's start: the ring goes
    // round it.
    let start_ring = match stretch {
        Some((_, Join::TurnsBack(turn))) => with_pad(&start_ring, &turn.pad).ok_or(End::OutOfRoom)?,
        _ => start_ring,
    };
    let plain = || {
        let distances = vec![EdgeOffset::even(sign * face); start_ring.len()];
        ring::offset(&start_ring, &distances).into_iter().next().ok_or(End::OutOfRoom)
    };
    let end_ring = match stretch {
        None => plain()?,
        Some((stretch, Join::Begins)) => beside_new_ramp(&start_ring, stretch, face, sign).ok_or(End::OutOfRoom)?,
        Some((stretch, _)) => spliced(&plain()?, stretch, face, sign).ok_or(End::OutOfRoom)?,
    };
    // The handover the next stretch starts from stays as it is.
    let keep: Vec<DVec2> = stretch
        .and_then(|(stretch, _)| Some(vec![stretch.edges.near.last()?.0, stretch.edges.far.last()?.0]))
        .unwrap_or_default();
    let end_ring = ring::filleted(&end_ring, spec.wall_radius, sign, &keep);
    let sound = !unsound(&start_ring, &end_ring, stretch.map(|(stretch, _)| stretch))
        && stretch.is_none_or(|(stretch, join)| {
            !crowded_handover(&end_ring, stretch)
                && arrives_on(&end_ring, stretch)
                && carries_on(from, stretch, join)
                && !pinched(stretch, spec.ramp.map_or(0.0, |ramp| ramp.width))
        });
    if !sound {
        return Err(End::OutOfRoom);
    }
    Ok((start_ring, end_ring))
}

/// The ring a bench ends on, where a stretch of ramp crosses it: the start
/// ring set off across the face (`plain`), with the stretch's length of it
/// replaced by the road's far edge set off by what is left of the face above
/// it, which is less the higher the road has climbed. Where the road
/// arrives, the ring crosses it straight to where its near edge arrived;
/// where it carries on from the stretch before, the far edge's offset runs
/// back until it meets the wall behind, the end of the berm the road takes
/// over, benched back like any other wall.
fn spliced(plain: &[DVec2], stretch: &Stretch, face: f64, sign: f64) -> Option<Vec<DVec2>> {
    let direction = stretch.direction;
    let far: Vec<DVec2> = stretch.edges.far.iter().map(|point| point.0).collect();
    let left_of_face: Vec<f64> = stretch
        .edges
        .far
        .iter()
        .map(|(_, source)| face * (1.0 - stretch.climbed_at(*source).unwrap_or(1.0)))
        .collect();
    let above = road::offset_chain(&far, &left_of_face, sign * direction > 0.0);
    let near_end = stretch.edges.near.last()?.0;
    if above.len() < 2 {
        return None;
    }
    let count = plain.len();
    let arcs = ring::arcs(plain);
    let around = arcs[count];
    let end_arc = ring::arc_at(&arcs, ring::nearest(plain, near_end).0);
    // How far round the plain ring a point is from the road's end, the way
    // the road runs.
    let from_end = |arc: f64| ((arc - end_arc) * direction).rem_euclid(around);

    // Where the ring leaves the plain one for the road: a point on it, how
    // far round it from the road's end, and the points of `above` to follow.
    let (leave, leave_arc, follow): (Vec<DVec2>, f64, &[DVec2]) = {
        // Where the far edge's offset first crosses the wall behind.
        let mut found: Option<(usize, DVec2, f64)> = None;
        'search: for (index, pair) in above.windows(2).enumerate() {
            for edge in 0..count {
                let (a, b) = (plain[edge], plain[(edge + 1) % count]);
                if let Some((point, t)) = crossing(pair[0], pair[1], a, b) {
                    let arc = arcs[edge] + (arcs[edge + 1] - arcs[edge]) * t;
                    found = Some((index, point, arc));
                    break 'search;
                }
            }
            // Only near the start: the wall behind is within a face and a
            // berm of it.
            if index > 64 {
                break;
            }
        }
        match found {
            Some((index, point, arc)) => (vec![point], arc, &above[index + 1..]),
            None => {
                let arc = ring::arc_at(&arcs, ring::nearest(plain, above[0]).0);
                (vec![], arc, &above[..])
            }
        }
    };

    // From where the road arrives, round the plain ring the way the road
    // runs to where it leaves for the road, then along the road to its end.
    let leave_from_end = from_end(leave_arc);
    let mut points: Vec<DVec2> = vec![ring::point_at(plain, ring::parameter_at(&arcs, end_arc))];
    let mut passed: Vec<(f64, DVec2)> = (0..count)
        .filter_map(|index| {
            let along = from_end(arcs[index]);
            (along > ring::SAME_POINT && along < leave_from_end - ring::SAME_POINT).then_some((along, plain[index]))
        })
        .collect();
    passed.sort_by(|a, b| a.0.total_cmp(&b.0));
    points.extend(passed.into_iter().map(|(_, point)| point));
    points.extend(leave);
    points.extend_from_slice(follow);
    if direction < 0.0 {
        points.reverse();
    }
    let ring = ring::cleaned(points);
    (ring.len() >= 3 && ring::signed_area(&ring) > 0.0).then_some(ring)
}

/// The ring a bench ends on where its ramp begins: the start ring set off
/// across the face, and the road's width further beside the stretch, which
/// is where the face above the road's far edge comes to. The step where the
/// road begins is wall, the head of its cut, and stands a face's run behind
/// the start, so the head wall rises at the face angle like any other.
/// Building down, the road's width reaches two faces' runs further back, so
/// the step comes to a face's run behind the start: the shelf the road
/// leaves stands on rock benched back under it rather than a face cut
/// beneath it.
fn beside_new_ramp(start_ring: &[DVec2], stretch: &Stretch, face: f64, sign: f64) -> Option<Vec<DVec2>> {
    let width = stretch.across;
    let direction = stretch.direction;
    let (near_start, near_end) = (stretch.edges.near.first()?.0, stretch.edges.near.last()?.0);
    let far_start = stretch.edges.far.first()?.0;
    let count = start_ring.len();
    let arcs = ring::arcs(start_ring);
    let around = arcs[count];
    let road_start = ring::arc_at(&arcs, ring::nearest(start_ring, near_start).0);
    let end_arc = ring::arc_at(&arcs, ring::nearest(start_ring, near_end).0);
    let with_step_at = |start_arc: f64| {
        let run = ((end_arc - start_arc) * direction).rem_euclid(around);
        let cuts = [ring::parameter_at(&arcs, start_arc), ring::parameter_at(&arcs, end_arc)];
        let (split, at) = ring::with_points_at(start_ring, &cuts);
        let distances: Vec<EdgeOffset> = (0..split.len())
            .map(|index| {
                let middle_arc = ring::arc_at(&arcs, middle_parameter(at[index], at[(index + 1) % split.len()], count));
                let along = ((middle_arc - start_arc) * direction).rem_euclid(around);
                EdgeOffset::even(sign * (face + if along < run { width } else { 0.0 }))
            })
            .collect();
        ring::offset(&split, &distances).into_iter().next()
    };
    // The head wall rises behind where the road starts, a face's run behind
    // its far edge at least: square over it, the wall would stand vertical.
    // Where the road starts at a corner, behind it is round the corner, and
    // the step goes further back until the road's start stands clear.
    let first = if sign < 0.0 { 2.0 } else { 1.0 };
    let mut last = None;
    for behind in [first, first + 1.0, first + 2.0, first + 4.0] {
        let ring = with_step_at(road_start - direction * behind * face)?;
        if ring::nearest(&ring, far_start).1 >= HEAD_WALL_CLEAR * face {
            return Some(ring);
        }
        last = Some(ring);
    }
    last
}

/// The least share of a face's run the head wall's top stands back from
/// where the road's far edge starts.
const HEAD_WALL_CLEAR: f64 = 0.5;

/// Where segment `a`-`b` crosses segment `c`-`d`, and how far along `c`-`d`.
fn crossing(a: DVec2, b: DVec2, c: DVec2, d: DVec2) -> Option<(DVec2, f64)> {
    match crate::model::kernel::segment_segment(a, b, c, d) {
        crate::model::kernel::SegSeg::Crossing { u, point, .. } | crate::model::kernel::SegSeg::Touching { u, point, .. } => Some((point, u)),
        _ => None,
    }
}

/// Whether a stretch lies clear of the ring it was laid along: neither edge
/// crosses it.
fn fits_against(ring: &[DVec2], stretch: &Stretch) -> bool {
    let ring_segments: Vec<(DVec2, DVec2)> = (0..ring.len()).map(|index| (ring[index], ring[(index + 1) % ring.len()])).collect();
    [&stretch.edges.near, &stretch.edges.far].iter().all(|edge| {
        let segments: Vec<(DVec2, DVec2)> = edge.windows(2).map(|pair| (pair[0].0, pair[1].0)).collect();
        !crosses(&segments, &ring_segments)
    })
}

/// Why a bench is not a sound piece of design, if it is not: its two rings
/// cross in plan, or its road crosses either. A design stops at the last
/// bench that is sound rather than going on with one that is not.
fn unsound(start_ring: &[DVec2], end_ring: &[DVec2], stretch: Option<&Stretch>) -> bool {
    let ring_segments = |ring: &[DVec2]| -> Vec<(DVec2, DVec2)> { (0..ring.len()).map(|index| (ring[index], ring[(index + 1) % ring.len()])).collect() };
    let chain_segments = |chain: &[(DVec2, f64)]| -> Vec<(DVec2, DVec2)> { chain.windows(2).map(|pair| (pair[0].0, pair[1].0)).collect() };
    let (start, end) = (ring_segments(start_ring), ring_segments(end_ring));
    crosses(&start, &end)
        || stretch.is_some_and(|stretch| {
            [&stretch.edges.near, &stretch.edges.far].iter().any(|edge| {
                let edge = chain_segments(edge);
                crosses(&edge, &start) || crosses(&edge, &end)
            })
        })
}

/// Whether the ring a stretch arrives on turns sharply within a road's
/// width of where it arrives (besides the two corners of the step across
/// the road itself): the next stretch would have to turn there as it
/// leaves, and its edges could not carry on from this one's.
fn crowded_handover(end_ring: &[DVec2], stretch: &Stretch) -> bool {
    let (Some(near_end), Some(far_end)) = (stretch.edges.near.last(), stretch.edges.far.last()) else {
        return false;
    };
    let count = end_ring.len();
    // The ring's turning near the handover, all of it: an arc turns a
    // little at each of many points.
    let (mut left, mut right) = (0.0f64, 0.0f64);
    for index in 0..count {
        let corner = end_ring[index];
        if corner.distance(near_end.0) > stretch.across || corner.distance(near_end.0) <= MEETS || corner.distance(far_end.0) <= MEETS {
            continue;
        }
        let (before, after) = (end_ring[(index + count - 1) % count], end_ring[(index + 1) % count]);
        let (Some(along_in), Some(along_out)) = ((corner - before).try_normalize(), (after - corner).try_normalize()) else {
            continue;
        };
        let turn = along_in.perp_dot(along_out).atan2(along_in.dot(along_out));
        if turn > 0.0 {
            left += turn;
        } else {
            right -= turn;
        }
    }
    left.max(right) > SHARP_HANDOVER
}

/// Whether the ring a bench ends on passes through where its stretch's two
/// edges arrived: the step across the road there is where the next stretch
/// carries on from.
fn arrives_on(end_ring: &[DVec2], stretch: &Stretch) -> bool {
    [stretch.edges.near.last(), stretch.edges.far.last()]
        .into_iter()
        .all(|point| point.is_some_and(|point| ring::nearest(end_ring, point.0).1 <= ARRIVES))
}

/// Whether a stretch starts where the last one arrived: its near edge on
/// the ring where the last near edge arrived, its far edge where the last
/// far edge did. A stretch laid off from the wrong side of a corner would
/// leave a gap in the road.
fn carries_on(from: &[DVec2], stretch: &Stretch, join: Join<'_>) -> bool {
    let Join::CarriesOn { handover, at } = join else {
        return true;
    };
    let (Some(near), Some(far)) = (stretch.edges.near.first(), stretch.edges.far.first()) else {
        return false;
    };
    near.0.distance(ring::point_at(from, at)) <= HANDOVER_GAP && far.0.distance(handover) <= HANDOVER_GAP
}

/// A bench's start ring, carried along a continuing stretch's far edge from
/// the handover to where it meets the ring: the berm beside the last
/// stretch ends against the new one, which climbs away from it across the
/// bench. Without this the berm's corner can cut across the road's edge
/// just past the handover.
fn along_far_edge(start_ring: Vec<DVec2>, stretch: &Stretch) -> Vec<DVec2> {
    let far: Vec<DVec2> = stretch.edges.far.iter().map(|point| point.0).collect();
    let count = start_ring.len();
    let (Some(&handover), true) = (far.first(), count >= 3) else {
        return start_ring;
    };
    let Some((at, gap)) = start_ring
        .iter()
        .enumerate()
        .map(|(index, point)| (index, point.distance(handover)))
        .min_by(|a, b| a.1.total_cmp(&b.1))
    else {
        return start_ring;
    };
    if gap > HANDOVER_GAP {
        return start_ring;
    }
    // The first place the far edge, walked from the handover, crosses the
    // ring, no further along it than a road's width.
    let mut walked = 0.0;
    let mut met = None;
    'walk: for (step, pair) in far.windows(2).enumerate() {
        if walked > stretch.across {
            break;
        }
        let mut first: Option<(f64, usize, DVec2)> = None;
        for edge in 0..count {
            if let crate::model::kernel::SegSeg::Crossing { t, point, .. } =
                crate::model::kernel::segment_segment(pair[0], pair[1], start_ring[edge], start_ring[(edge + 1) % count])
                && (step > 0 || t > 1.0e-6)
                && first.is_none_or(|(best, _, _)| t < best)
            {
                first = Some((t, edge, point));
            }
        }
        if let Some((_, edge, point)) = first {
            met = Some((step, edge, point));
            break 'walk;
        }
        walked += pair[0].distance(pair[1]);
    }
    let Some((step, edge, point)) = met else {
        return start_ring;
    };
    // From the handover straight to where the far edge meets the ring, a
    // hair to the side away from the road: past the handover the far edge
    // has climbed away from the ring's level, so the two must not share a
    // point in plan.
    let (a, b) = (far[step], far[step + 1]);
    let Some(across_edge) = (b - a).try_normalize().map(DVec2::perp) else {
        return start_ring;
    };
    let near = stretch.edges.near.get(step.min(stretch.edges.near.len().saturating_sub(1))).map_or(a, |point| point.0);
    let away = if (point - near).dot(across_edge) >= 0.0 { across_edge } else { -across_edge };
    let path = [handover, point + away * BESIDE_ROAD];
    // Replace whichever way round the ring from the handover to there is
    // shorter.
    let span = |from: usize, to: usize| -> f64 {
        (0..(to + count - from) % count)
            .map(|k| start_ring[(from + k) % count].distance(start_ring[(from + k + 1) % count]))
            .sum()
    };
    let forward = span(at, edge) + start_ring[edge].distance(point);
    let backward = span((edge + 1) % count, at) + point.distance(start_ring[(edge + 1) % count]);
    let mut ring = Vec::with_capacity(count + path.len());
    if forward <= backward {
        // The ring ran handover .. edge, then on to the crossing: from the
        // crossing round the rest, then the far edge back up to it.
        let mut k = (edge + 1) % count;
        while k != at {
            ring.push(start_ring[k]);
            k = (k + 1) % count;
        }
        ring.extend(path.iter().copied());
    } else {
        // The ring ran from the crossing on to the handover: the far edge
        // taken the other way.
        let mut k = (at + 1) % count;
        loop {
            ring.push(start_ring[k]);
            if k == edge {
                break;
            }
            k = (k + 1) % count;
        }
        ring.extend(path.iter().rev().copied());
    }
    ring::cleaned(ring)
}

/// How far beside a road's edge a ring passes where it meets it away from
/// the road's own level.
const BESIDE_ROAD: f64 = 0.05;

/// Whether a stretch's road is narrower anywhere than its width (less a
/// little): its far edge cut back by the wall across a corner it turns at
/// its end, say, so the road pinches before it arrives.
fn pinched(stretch: &Stretch, width: f64) -> bool {
    let far = &stretch.edges.far;
    let narrowest = width * PINCH;
    stretch.edges.near.iter().any(|(point, _)| {
        far.windows(2)
            .map(|pair| crate::model::kernel::project_onto_segment(*point, pair[0].0, pair[1].0).0.distance(*point))
            .fold(f64::INFINITY, f64::min)
            < narrowest
    })
}

/// The share of its width a road may narrow to and still be sound.
const PINCH: f64 = 0.96;

/// How far a stretch's edge may start from where the last one arrived; the
/// edge is joined straight on to it.
const HANDOVER_GAP: f64 = 1.0;

/// How near the ring a bench ends on must pass where its road arrives.
const ARRIVES: f64 = 0.01;

/// A ring turning more than this either way near a handover is too sharp
/// for the ramp to carry on from there.
const SHARP_HANDOVER: f64 = std::f64::consts::PI / 6.0;

/// Whether any segment of one set properly crosses one of the other.
fn crosses(first: &[(DVec2, DVec2)], second: &[(DVec2, DVec2)]) -> bool {
    first.iter().any(|(a, b)| {
        let (low, high) = (a.min(*b), a.max(*b));
        second.iter().any(|(c, d)| {
            c.min(*d).cmple(high).all()
                && c.max(*d).cmpge(low).all()
                && matches!(crate::model::kernel::segment_segment(*a, *b, *c, *d), crate::model::kernel::SegSeg::Crossing { .. })
        })
    })
}

/// The parameter half way between two, going forward from `start`.
fn middle_parameter(start: f64, end: f64, count: usize) -> f64 {
    let span = (end - start).rem_euclid(count as f64);
    (start + span * 0.5).rem_euclid(count as f64)
}

/// The point on a chain where it was set off from a ring parameter.
fn point_by_source(chain: &[(DVec2, f64)], parameter: f64, count: usize) -> DVec2 {
    let wrap = count as f64;
    let mut best = (chain[0].0, f64::INFINITY);
    for pair in chain.windows(2) {
        let (a, mut b) = (pair[0].1, pair[1].1);
        if (b - a).abs() > wrap * 0.5 {
            b += if b < a { wrap } else { -wrap };
        }
        let mut wanted = parameter;
        if (wanted - a).abs() > wrap * 0.5 {
            wanted += if wanted < a { wrap } else { -wrap };
        }
        let (low, high) = (a.min(b), a.max(b));
        if wanted >= low - 1.0e-9 && wanted <= high + 1.0e-9 {
            let t = if (b - a).abs() > 1.0e-12 { (wanted - a) / (b - a) } else { 0.0 };
            return pair[0].0.lerp(pair[1].0, t.clamp(0.0, 1.0));
        }
        for (point, source) in [pair[0], pair[1]] {
            let gap = (source - parameter).abs().min(wrap - (source - parameter).abs());
            if gap < best.1 {
                best = (point, gap);
            }
        }
    }
    best.0
}

/// Append a stretch's `points` to a string. It starts where the last
/// stretch arrived, or near enough that its first point is dropped and the
/// string runs straight on from the last's end.
fn join(string: &mut Vec<DVec3>, points: Vec<DVec3>) {
    let skip = match (string.last(), points.first()) {
        (Some(last), Some(first)) => usize::from(last.distance(*first) <= HANDOVER_GAP && points.len() > 1),
        _ => 0,
    };
    string.extend(points.into_iter().skip(skip));
}

/// A ring as a level string: a toe or a crest by which way the design is
/// built and whether it starts a bench.
fn level_string(spec: &Spec, ring: &[DVec2], elevation: f64, starts_bench: bool) -> DesignString {
    let kind = match (spec.direction, starts_bench) {
        (Direction::Up, true) | (Direction::Down, false) => StringKind::Toe,
        _ => StringKind::Crest,
    };
    DesignString {
        kind,
        points: ring.iter().map(|point| DVec3::new(point.x, point.y, elevation)).collect(),
        closed: true,
    }
}

/// Volume between two level outlines `height` apart, taken as a frustum.
fn frustum(area_a: f64, area_b: f64, height: f64) -> f64 {
    let (a, b) = (area_a.abs(), area_b.abs());
    height / 3.0 * (a + b + (a * b).sqrt())
}
