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

pub(crate) mod checks;
pub(crate) mod evaluation;
pub(crate) mod object;
#[cfg(test)]
mod object_tests;
pub(crate) mod ring;
pub(crate) mod road;
pub(crate) mod settings;
#[cfg(test)]
mod tests;
pub(crate) mod walls;

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

/// What a design is of: a pit cut into the ground, or a dump or stockpile
/// built on it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Kind {
    #[default]
    Pit,
    /// Built up from its toe stepping in, or down from its top stepping out
    /// until it meets the ground: a pit turned upside down.
    Dump,
}

impl Direction {
    /// The other way.
    fn flipped(self) -> Self {
        match self {
            Self::Up => Self::Down,
            Self::Down => Self::Up,
        }
    }

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
    /// Back the way it came, every `every` benches (or only at the RLs its
    /// ramp asks, at 0); the road's inside edge turns no tighter than
    /// `inside_radius`, and the road is `widening` wider round the middle of
    /// the turn than its width, for the trucks to make it. Level on a pad
    /// at the bench's start at a `grade` of 0; otherwise climbing round the
    /// turn, its inside edge at that grade, the stretch after it climbing
    /// what is left.
    Switchback { every: u32, inside_radius: f64, widening: f64, grade: f64 },
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RampSpec {
    pub(crate) width: f64,
    /// Rise over run, held along its steeper edge.
    pub(crate) gradient: f64,
    /// Where it leaves the starting string: the point on it nearest this.
    pub(crate) start: DVec2,
    /// Whether it climbs clockwise seen from above.
    pub(crate) climbs_clockwise: bool,
    pub(crate) layout: Layout,
    /// The RL it begins at and the RL it ends at, the way the design is
    /// built: on the benches that start between them. The whole pit where
    /// none is given.
    pub(crate) from: Option<f64>,
    pub(crate) to: Option<f64>,
    /// Bands of RL whose benches' stretches of road are laid otherwise.
    pub(crate) bands: Vec<RampBand>,
    /// How far it runs level where it leaves the pit: at its top, building
    /// up, before it climbs out onto the last crest; at its first bench,
    /// building down, where it starts from the crest.
    pub(crate) exit_flat: f64,
    /// How far each stretch runs level where it leaves its bench's berm, so
    /// the berm is reached from the road on the flat: at least this, or what
    /// its band asks if more.
    pub(crate) junction_flat: f64,
    /// RLs it turns back at, where it switches back: on the first bench
    /// starting at or past each, the way the design is built, where a turn
    /// fits there; on the next that has room where not.
    pub(crate) turns_at: Vec<f64>,
    /// The tightest its inside edge should turn anywhere, for the trucks:
    /// what a switchback turns round, and what the road is checked against
    /// everywhere else.
    pub(crate) turning_radius: f64,
}

/// A band of RL whose benches' stretches of road are laid otherwise: as
/// wide as it says (a one-lane bottom, say), and running level at least as
/// far as it says before they climb.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct RampBand {
    pub(crate) from: f64,
    pub(crate) to: f64,
    pub(crate) width: Option<f64>,
    pub(crate) flat: Option<f64>,
}

impl RampSpec {
    /// Whether the ramp is on a bench starting at `elevation`, built the
    /// way `sign` says.
    fn holds(&self, elevation: f64, sign: f64) -> bool {
        let into = elevation + sign * 1.0e-6;
        self.from.is_none_or(|from| (into - from) * sign > 0.0) && self.to.is_none_or(|to| (to - into) * sign > 0.0)
    }

    /// The band a bench starting at `elevation` is laid by, if any: the
    /// last listed that holds it.
    pub(crate) fn band(&self, elevation: f64, sign: f64) -> Option<&RampBand> {
        self.bands.iter().rev().find(|band| {
            Zone {
                from: band.from,
                to: band.to,
                ..Zone::default()
            }
            .holds(elevation, sign)
        })
    }

    /// The same ramp for heights turned over, as a dump is designed: it
    /// climbs the other way round the pit it is designed as.
    fn turned_over(&self) -> Self {
        Self {
            climbs_clockwise: !self.climbs_clockwise,
            from: self.from.map(|from| -from),
            to: self.to.map(|to| -to),
            turns_at: self.turns_at.iter().map(|rl| -rl).collect(),
            bands: self
                .bands
                .iter()
                .map(|band| RampBand {
                    from: -band.to,
                    to: -band.from,
                    ..*band
                })
                .collect(),
            ..self.clone()
        }
    }
}

/// A bench's ramp: where it is, and whether the ramp leaves the pit on it.
#[derive(Clone, Copy, Debug)]
struct RampOn {
    cursor: RampCursor,
    exits: bool,
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

/// How the benches vary from one to the next, beyond the design's own
/// height, face angle and berm width.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct BenchRules {
    /// Heights the benches take in turn from the base string, over and over:
    /// 10, 10, 15 makes every third bench 15 high. None gives every bench
    /// the design's height.
    pub(crate) height_cycle: Vec<f64>,
    /// Every this many benches, the berm is `catch_width` wide instead of
    /// the design's: a catch berm, or with no width, no berm at all. None
    /// at 0.
    pub(crate) catch_every: u32,
    pub(crate) catch_width: f64,
    /// Bands of RL whose benches are cut otherwise, where the rock calls for
    /// it. Where bands overlap, the one listed last counts.
    pub(crate) zones: Vec<Zone>,
    /// The face angle and berm width walls take by the way they face, round
    /// the compass. None: the design's (or the zone's) all round.
    pub(crate) sectors: Vec<walls::Sector>,
    /// Areas in plan whose walls are cut otherwise. Where they overlap, the
    /// one listed last counts.
    pub(crate) regions: Vec<walls::Region>,
    /// How far across a region's edge its values blend into those round it.
    pub(crate) blend: f64,
    /// The inter-ramp angle the walls should come to, in degrees, to check
    /// the design against where a sector sets none.
    pub(crate) inter_ramp: Option<f64>,
}

impl Default for BenchRules {
    fn default() -> Self {
        Self {
            height_cycle: Vec::new(),
            catch_every: 0,
            catch_width: 0.0,
            zones: Vec::new(),
            sectors: Vec::new(),
            regions: Vec::new(),
            blend: 20.0,
            inter_ramp: None,
        }
    }
}

/// A band of RL whose benches are cut otherwise than the design's: each
/// value given replaces the design's own for the benches that start in it,
/// and the berms on its levels.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct Zone {
    pub(crate) from: f64,
    pub(crate) to: f64,
    pub(crate) height: Option<f64>,
    /// Degrees from horizontal.
    pub(crate) face_angle: Option<f64>,
    pub(crate) berm_width: Option<f64>,
    /// The tightest its walls may turn.
    pub(crate) wall_radius: Option<f64>,
}

impl Default for Zone {
    fn default() -> Self {
        Self {
            from: 0.0,
            to: 0.0,
            height: None,
            face_angle: None,
            berm_width: None,
            wall_radius: None,
        }
    }
}

impl Zone {
    /// Whether a bench starting at `elevation`, built the way `sign` says,
    /// starts in the band: a bench starting on one of its edges is in it if
    /// it is built into it.
    fn holds(&self, elevation: f64, sign: f64) -> bool {
        let (low, high) = (self.from.min(self.to), self.from.max(self.to));
        let into = elevation + sign * 1.0e-6;
        into > low && into < high
    }
}

impl BenchRules {
    /// The same rules for heights turned over, as a dump is designed: its
    /// walls, which face out, are designed as a pit's facing in.
    fn turned_over(&self) -> Self {
        Self {
            sectors: self
                .sectors
                .iter()
                .map(|sector| walls::Sector {
                    bearing: (sector.bearing + 180.0).rem_euclid(360.0),
                    ..*sector
                })
                .collect(),
            zones: self
                .zones
                .iter()
                .map(|zone| Zone {
                    from: -zone.to,
                    to: -zone.from,
                    ..*zone
                })
                .collect(),
            ..self.clone()
        }
    }
}

/// How one bench is cut: its height, what its zone sets, and the berm it
/// leaves whatever the wall; the walls themselves vary as [`walls::Walls`]
/// says.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Cut {
    height: f64,
    zone_face: Option<f64>,
    zone_berm: Option<f64>,
    fixed_berm: Option<f64>,
    /// The tightest its walls may turn.
    wall_radius: f64,
}

impl Cut {
    fn walls<'a>(&self, spec: &'a Spec) -> walls::Walls<'a> {
        walls::Walls {
            spec,
            height: self.height,
            zone_face: self.zone_face,
            zone_berm: self.zone_berm,
            fixed_berm: self.fixed_berm,
            wall_radius: self.wall_radius,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Spec {
    pub(crate) kind: Kind,
    pub(crate) direction: Direction,
    pub(crate) bench_height: f64,
    /// Bench face angle from horizontal, in degrees.
    pub(crate) face_angle: f64,
    pub(crate) berm_width: f64,
    pub(crate) rules: BenchRules,
    /// The tightest a wall may turn where the design would otherwise leave
    /// a sharp nose of rock (building up) or a sharp corner of pit (building
    /// down); also what keeps the ramp from turning tighter than its walls.
    pub(crate) wall_radius: f64,
    /// The ramps, each laid bench by bench on the benches it runs on.
    pub(crate) ramps: Vec<RampSpec>,
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
        if !(self.wall_radius.is_finite() && self.wall_radius >= 0.0) {
            return Some(Problem::WallRadius);
        }
        let rules = &self.rules;
        let angle = |value: f64| value.is_finite() && value > 0.0 && value < 90.0;
        let width = |value: f64| value.is_finite() && value >= 0.0;
        if !rules.height_cycle.iter().all(|height| positive(*height)) || !rules.zones.iter().all(|zone| zone.height.is_none_or(positive)) {
            return Some(Problem::BenchHeight);
        }
        if !rules.zones.iter().all(|zone| zone.wall_radius.is_none_or(width)) {
            return Some(Problem::WallRadius);
        }
        if !rules.zones.iter().all(|zone| zone.face_angle.is_none_or(angle)) {
            return Some(Problem::FaceAngle);
        }
        if !width(rules.catch_width) || !rules.zones.iter().all(|zone| zone.berm_width.is_none_or(width)) {
            return Some(Problem::BermWidth);
        }
        if !rules.zones.iter().all(|zone| zone.from.is_finite() && zone.to.is_finite() && zone.from != zone.to) {
            return Some(Problem::Zone);
        }
        if !rules
            .sectors
            .iter()
            .all(|sector| sector.bearing.is_finite() && angle(sector.face_angle) && width(sector.berm_width) && sector.inter_ramp.is_none_or(angle))
            || !rules.inter_ramp.is_none_or(angle)
        {
            return Some(Problem::Sector);
        }
        if !width(rules.blend)
            || !rules.regions.iter().all(|region| {
                region.outline.len() >= 3 && region.outline.iter().all(|point| point.is_finite()) && region.face_angle.is_none_or(angle) && region.berm_width.is_none_or(width)
            })
        {
            return Some(Problem::Region);
        }
        for ramp in &self.ramps {
            if !positive(ramp.width) || !ramp.bands.iter().all(|band| band.width.is_none_or(positive)) {
                return Some(Problem::RampWidth);
            }
            let length = |value: f64| value.is_finite() && value >= 0.0;
            if !length(ramp.exit_flat)
                || !length(ramp.junction_flat)
                || !ramp
                    .bands
                    .iter()
                    .all(|band| band.flat.is_none_or(length) && band.from.is_finite() && band.to.is_finite() && band.from != band.to)
                || !ramp.from.is_none_or(f64::is_finite)
                || !ramp.to.is_none_or(f64::is_finite)
            {
                return Some(Problem::RampBand);
            }
            if !positive(ramp.gradient) {
                return Some(Problem::RampGradient);
            }
            if let Layout::Switchback {
                every,
                inside_radius,
                widening,
                grade,
            } = ramp.layout
                && ((every == 0 && ramp.turns_at.is_empty())
                    || !(inside_radius.is_finite() && inside_radius >= 0.0)
                    || !(widening.is_finite() && widening >= 0.0)
                    || !(grade.is_finite() && grade >= 0.0 && grade <= ramp.gradient)
                    || !ramp.turns_at.iter().all(|rl| rl.is_finite()))
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

    /// The band of RL a bench starting at `elevation` is cut by, if any.
    fn zone(&self, elevation: f64) -> Option<&Zone> {
        let sign = self.direction.sign();
        self.rules.zones.iter().rev().find(|zone| zone.holds(elevation, sign))
    }

    /// How bench `index`, counting from the base string, is cut where it
    /// starts at `elevation`: its height before any stop cuts it short, its
    /// face angle, and the berm left before it (none before the first).
    fn cut(&self, index: u32, elevation: f64) -> Cut {
        let zone = self.zone(elevation);
        let cycled = match self.rules.height_cycle.as_slice() {
            [] => self.bench_height,
            cycle => cycle[index as usize % cycle.len()],
        };
        let catch = self.rules.catch_every > 0 && index > 0 && index.is_multiple_of(self.rules.catch_every);
        Cut {
            height: zone.and_then(|zone| zone.height).unwrap_or(cycled),
            zone_face: zone.and_then(|zone| zone.face_angle),
            zone_berm: zone.and_then(|zone| zone.berm_width),
            wall_radius: zone.and_then(|zone| zone.wall_radius).unwrap_or(self.wall_radius),
            fixed_berm: match (index, catch) {
                (0, _) => Some(0.0),
                (_, true) => Some(self.rules.catch_width),
                (_, false) => None,
            },
        }
    }
}

/// The horizontal run of a face `height` high at `face_angle` degrees.
fn face_run(height: f64, face_angle: f64) -> f64 {
    height / face_angle.to_radians().tan()
}

/// Why settings cannot make a design.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Problem {
    BenchHeight,
    /// A band of RL with no height to it.
    Zone,
    /// A wall radius below 0.
    WallRadius,
    /// A ramp band with no height to it, or a flat below 0.
    RampBand,
    /// A slope sector's values cannot cut a wall.
    Sector,
    /// A slope region with no outline, or values that cannot cut a wall.
    Region,
    FaceAngle,
    BermWidth,
    RampWidth,
    RampGradient,
    Switchback,
    Benches,
    /// Daylighting builds up to the ground; a design built down has none.
    DaylightDown,
    /// A dump meets the ground built down from its top; built up it has
    /// none to meet.
    DaylightDumpUp,
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
    /// Where the design meets the topography it is measured against.
    Daylight,
}

impl StringKind {
    /// The colour a string is drawn in, by what it is.
    pub(crate) fn colour(self) -> [f32; 4] {
        match self {
            Self::Toe => [0.30, 0.60, 0.95, 1.0],
            Self::Crest => [0.90, 0.35, 0.25, 1.0],
            Self::RampEdge => [0.35, 0.80, 0.40, 1.0],
            Self::RampCentreline => [0.62, 0.66, 0.62, 1.0],
            Self::Daylight => [0.95, 0.88, 0.25, 1.0],
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
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct BenchReport {
    /// The RL it starts from.
    pub(crate) elevation: f64,
    pub(crate) height: f64,
    /// The least and greatest of its face angles round the pit, in degrees.
    pub(crate) face_angle: [f64; 2],
    /// The least and greatest width of the berm left before it, at its
    /// starting RL.
    pub(crate) berm: [f64; 2],
    /// The plan area inside the ring it ends on.
    pub(crate) area: f64,
    /// What each ramp on it came to, in the order the ramps are listed.
    pub(crate) ramps: Vec<RampReport>,
}

#[cfg(test)]
impl BenchReport {
    /// What the first ramp on it came to, if any ramp is on it.
    pub(crate) fn ramp(&self) -> Option<RampReport> {
        self.ramps.first().copied()
    }
}

/// What a bench's stretch of ramp came to.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct RampReport {
    /// Which ramp, by its place in the list.
    #[serde(default)]
    pub(crate) ramp: usize,
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
    /// Where it turned back later than asked, for want of room: the RL of
    /// the bench the turn was due on.
    #[serde(default)]
    pub(crate) turn_moved_from: Option<f64>,
    /// Where it turned back, if it did: the middle of the turn, on its pad.
    #[serde(default)]
    pub(crate) turn_at: Option<DVec3>,
    /// How far it climbed round the turn it turned back on: none on a level
    /// pad.
    #[serde(default)]
    pub(crate) turn_rise: f64,
    /// Where its steepest metre is: the middle of it, on the edge it is on.
    #[serde(default)]
    pub(crate) steepest_at: Option<DVec3>,
    /// Where it is narrowest: half way across it there.
    #[serde(default)]
    pub(crate) narrowest_at: Option<DVec3>,
    /// Half way along it, on its near edge.
    #[serde(default)]
    pub(crate) middle: Option<DVec3>,
}

impl RampReport {
    /// Every place it names, to move or turn over with the design.
    fn places_mut(&mut self) -> impl Iterator<Item = &mut DVec3> {
        [&mut self.turn_at, &mut self.steepest_at, &mut self.narrowest_at, &mut self.middle].into_iter().flatten()
    }
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
    /// Whether it is a dump's, built on the ground rather than cut into it.
    #[serde(default)]
    pub(crate) dump: bool,
    /// What the walls come to overall, facing each way.
    #[serde(default)]
    pub(crate) walls: Vec<walls::WallReport>,
    /// Where each ramp leaves the pit, in the order they are listed.
    #[serde(default)]
    pub(crate) ramp_exits: Vec<Option<DVec3>>,
    /// The ramp that found no room, where the design stopped for want of
    /// it.
    #[serde(default)]
    pub(crate) blocked_ramp: Option<usize>,
    /// Where it falls short of what was asked, each place once.
    #[serde(default)]
    pub(crate) flags: Vec<checks::Flag>,
}

impl Design {
    /// Each ramp's road as a closed outline: along one edge and back along
    /// the other.
    pub(crate) fn road_outlines(&self) -> Vec<Vec<DVec3>> {
        // A road's edges are written one after the other, left then right.
        let edges: Vec<&DesignString> = self.strings.iter().filter(|string| string.kind == StringKind::RampEdge).collect();
        edges
            .as_chunks::<2>()
            .0
            .iter()
            .map(|[left, right]| left.points.iter().copied().chain(right.points.iter().rev().copied()).collect())
            .collect()
    }

    /// The RL of the last ring built.
    pub(crate) fn final_elevation(&self) -> f64 {
        self.levels.last().copied().unwrap_or_default()
    }

    /// The design with every height turned over, as a dump is the pit it is
    /// upside down.
    fn turned_over(mut self) -> Self {
        for string in &mut self.strings {
            for point in &mut string.points {
                point.z = -point.z;
            }
        }
        for level in &mut self.levels {
            *level = -*level;
        }
        for bench in &mut self.report {
            bench.elevation = -bench.elevation;
            for road in &mut bench.ramps {
                road.turn_moved_from = road.turn_moved_from.map(|rl| -rl);
                for place in road.places_mut() {
                    place.z = -place.z;
                }
            }
        }
        // A dump's walls face out, where the pit it was designed as faces in.
        for wall in &mut self.walls {
            wall.bearing = (wall.bearing + 180.0).rem_euclid(360.0);
            if let Some(place) = &mut wall.steepest_at {
                place.z = -place.z;
            }
        }
        for flag in &mut self.flags {
            flag.at.z = -flag.at.z;
        }
        self.ramp_exit = self.ramp_exit.map(|exit| DVec3::new(exit.x, exit.y, -exit.z));
        for exit in self.ramp_exits.iter_mut().flatten() {
            exit.z = -exit.z;
        }
        self.dump = true;
        self
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
            for road in &mut bench.ramps {
                road.turn_moved_from = road.turn_moved_from.map(|rl| rl + delta.z);
                for place in road.places_mut() {
                    *place += delta;
                }
            }
        }
        for place in moved.walls.iter_mut().filter_map(|wall| wall.steepest_at.as_mut()) {
            *place += delta;
        }
        for flag in &mut moved.flags {
            flag.at += delta;
        }
        moved.ramp_exit = moved.ramp_exit.map(|exit| exit + delta);
        for exit in moved.ramp_exits.iter_mut().flatten() {
            *exit += delta;
        }
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
    /// The RL of the bench it began on.
    begun_at: f64,
    /// How many of the turns its ramp asks for at RLs it has made.
    asked_made: u32,
    /// The RL of the bench a turn back fell due on, where none has fitted
    /// since.
    due_since: Option<f64>,
    /// Which way in plan the road was heading as it arrived at `at`.
    heading: DVec2,
}

/// A bench's stretch of ramp as laid, with the switchback it turned back on
/// to start, if it did, and how far its start was moved to fit.
struct Laid {
    /// Which ramp, by its place in the list.
    index: usize,
    stretch: Stretch,
    turn: Option<Turn>,
    /// Where the last stretch of its ramp arrived, if it carries on from
    /// one: its far edge's end, and where its near edge ended on the ring.
    handover: Option<(DVec2, f64)>,
    /// Whether it was laid along the bench's start ring, beginning on its
    /// berm.
    on_start: bool,
    /// The flat its ramp's band, or its leaving the pit, asked of it.
    base_flat: f64,
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
    /// How far it climbs round the turn: none on a level pad.
    lift: f64,
}

impl Turn {
    /// A chain of the turn at its heights, climbing `lift` from `elevation`
    /// along it the way `sign` says.
    fn at_height(&self, chain: &[DVec2], elevation: f64, sign: f64) -> Vec<DVec3> {
        let total: f64 = chain.windows(2).map(|pair| pair[0].distance(pair[1])).sum();
        let mut run = 0.0;
        chain
            .iter()
            .enumerate()
            .map(|(index, point)| {
                if index > 0 {
                    run += point.distance(chain[index - 1]);
                }
                let share = if total > 0.0 { run / total } else { 0.0 };
                point.extend(elevation + sign * self.lift * share)
            })
            .collect()
    }
}

/// The height a stretch of road `height` high is at, `climbed` of the way
/// up it, after a turn that climbed `lift` of it.
fn stretch_height(elevation: f64, sign: f64, height: f64, lift: f64, climbed: f64) -> f64 {
    elevation + sign * (lift + (height - lift) * climbed)
}

/// The ground's RL at a point, for daylighting.
pub(crate) type Ground<'a> = &'a dyn Fn(DVec2) -> Option<f64>;

/// Build the design a base string and its settings describe, stopping early
/// if `cancelled` says to.
pub(crate) fn design(base: &Base, spec: &Spec, ground: Option<Ground<'_>>, cancelled: &dyn Fn() -> bool) -> Result<Design, Problem> {
    if spec.kind == Kind::Dump {
        return design_dump(base, spec, ground, cancelled);
    }
    design_pit(base, spec, ground, cancelled)
}

/// A dump, designed as the pit it is upside down: built up stepping in, it
/// is a pit built down stepping in with every height turned over; built
/// down stepping out to the ground, a pit built up to daylight. The ramp
/// that climbs one way round the dump falls that way round the pit, so it is
/// asked to climb the other.
fn design_dump(base: &Base, spec: &Spec, ground: Option<Ground<'_>>, cancelled: &dyn Fn() -> bool) -> Result<Design, Problem> {
    let flipped = Spec {
        kind: Kind::Pit,
        direction: spec.direction.flipped(),
        ramps: spec.ramps.iter().map(RampSpec::turned_over).collect(),
        stop: match spec.stop {
            Stop::Elevation(elevation) => Stop::Elevation(-elevation),
            stop => stop,
        },
        rules: spec.rules.turned_over(),
        ..spec.clone()
    };
    let base = Base {
        ring: base.ring.clone(),
        elevation: -base.elevation,
        unevenness: base.unevenness,
    };
    let below = |point: DVec2| ground.and_then(|ground| ground(point)).map(|z| -z);
    let ground: Option<Ground<'_>> = ground.map(|_| &below as Ground<'_>);
    match design_pit(&base, &flipped, ground, cancelled) {
        Ok(design) => Ok(design.turned_over()),
        Err(Problem::DaylightDown) => Err(Problem::DaylightDumpUp),
        Err(problem) => Err(problem),
    }
}

/// A pit, cut into the ground.
fn design_pit(base: &Base, spec: &Spec, ground: Option<Ground<'_>>, cancelled: &dyn Fn() -> bool) -> Result<Design, Problem> {
    // A ramp kept to its gradient by flats can come to a bench with no room
    // where one climbing a little flatter would have found it, the benches
    // before having put it somewhere else; and conditioned rings, a hair
    // different, can put it somewhere with no way on where rings left as
    // they were would not. Where a design runs out of room, the others are
    // tried in turn: the one that gets furthest is given, the first of
    // those that get as far.
    let mut best = design_with(base, spec, ground, cancelled, Additions::FlatsFirst, true)?;
    for (additions, condition) in [(Additions::LongerOnly, true), (Additions::FlatsFirst, false), (Additions::LongerOnly, false)] {
        if best.end != End::OutOfRoom || cancelled() {
            break;
        }
        if additions == Additions::LongerOnly && spec.ramps.is_empty() {
            continue;
        }
        let other = design_with(base, spec, ground, cancelled, additions, condition)?;
        if other.benches > best.benches {
            best = other;
        }
    }
    Ok(best)
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

/// A design built with only `additions` to its stretches, its end rings
/// conditioned where `condition` says.
fn design_with(base: &Base, spec: &Spec, ground: Option<Ground<'_>>, cancelled: &dyn Fn() -> bool, additions: Additions, condition: bool) -> Result<Design, Problem> {
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
            condition,
            strings: vec![level_string(spec, &start, base.elevation, true)],
            roads: vec![Road::default(); spec.ramps.len()],
            ramp_length: 0.0,
            start_moved: 0.0,
            built: Built::default(),
            blocked: None,
        },
        // Each begun on the first bench it is on.
        ramps: vec![None; spec.ramps.len()],
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
        let Some(cut) = state.next_cut() else {
            break End::Complete;
        };
        let snapshot = state.clone();
        match state.builder.bench(&state.current, state.elevation, cut, state.ramps_for(&cut), None) {
            Ok(bench) => {
                let added = bench.added;
                state.advance(bench, cut);
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
    let Builder {
        mut strings,
        roads,
        ramp_length,
        start_moved,
        blocked,
        ..
    } = builder;
    // Where each ramp leaves the pit: its top building up, where building
    // down it began.
    let ramp_exits: Vec<Option<DVec3>> = roads
        .iter()
        .map(|road| match spec.direction {
            Direction::Up => road.centreline.last().copied(),
            Direction::Down => road.centreline.first().copied(),
        })
        .collect();
    for road in &roads {
        for (points, kind) in [
            (&road.left, StringKind::RampEdge),
            (&road.right, StringKind::RampEdge),
            (&road.centreline, StringKind::RampCentreline),
        ] {
            if points.len() >= 2 {
                strings.push(DesignString {
                    kind,
                    points: points.clone(),
                    closed: false,
                });
            }
        }
    }
    let mut design = Design {
        strings,
        benches,
        report,
        levels,
        end,
        ramp_length,
        ramp_start_moved: start_moved,
        ramp_exit: ramp_exits.first().copied().flatten(),
        ramp_exits,
        blocked_ramp: blocked.filter(|_| end == End::OutOfRoom),
        volume,
        dump: false,
        walls: Vec::new(),
        flags: Vec::new(),
    };
    without_straight_points(&mut design.strings);
    design.walls = inter_ramp(spec, &base.ring, &design);
    design.flags = checks::flags(spec, &design, &roads);
    Ok(design)
}

/// A finished design's rings with the points that do not bend them taken
/// out (see [`ring::without_straight_points`]): the same rings, lighter.
/// The string the design starts from is the user's own, and stays.
fn without_straight_points(strings: &mut [DesignString]) {
    for string in strings.iter_mut().filter(|string| string.closed).skip(1) {
        let z = string.points.first().map_or(0.0, |point| point.z);
        let plan: Vec<DVec2> = string.points.iter().map(|point| point.truncate()).collect();
        string.points = ring::without_straight_points(&plan).into_iter().map(|point| point.extend(z)).collect();
    }
}

/// How near its road a bench's end ring is left as it is in conditioning
/// it.
const CONDITION_CLEAR: f64 = 2.0;

/// How far apart round the string the walls' inter-ramp angle is measured.
const INTER_RAMP_STEP: f64 = 10.0;

/// What the walls of a design come to between ramps, facing each way.
///
/// Every [`INTER_RAMP_STEP`] round the string the design starts from, a
/// line is run square out across the walls to the last ring. Where it
/// crosses the ramp, the walls are cut into stacks of benches; each stack
/// of two benches or more is measured from the ring its lowest bench starts
/// on to the ring its highest ends on (toe to crest, building up), and set
/// against the angle those benches were cut to make there. Grouped by slope
/// sector, or where there are none, by compass octant.
fn inter_ramp(spec: &Spec, base: &[DVec2], design: &Design) -> Vec<walls::WallReport> {
    let Some(last) = design.strings.iter().rev().find(|string| string.closed) else {
        return Vec::new();
    };
    let last: Vec<DVec2> = last.points.iter().map(|point| point.truncate()).collect();
    let rules = &spec.rules;
    let (bins, targets): (Vec<f64>, Vec<Option<f64>>) = if rules.sectors.is_empty() {
        (0..8).map(|octant| (f64::from(octant) * 45.0, rules.inter_ramp)).unzip()
    } else {
        rules
            .sectors
            .iter()
            .map(|sector| (sector.bearing.rem_euclid(360.0), sector.inter_ramp.or(rules.inter_ramp)))
            .unzip()
    };
    // How each bench was cut, its height as built.
    let cuts: Vec<Cut> = design
        .report
        .iter()
        .enumerate()
        .map(|(index, bench)| Cut {
            height: bench.height,
            ..spec.cut(u32::try_from(index).unwrap_or(u32::MAX), bench.elevation)
        })
        .collect();
    let tallest = cuts.iter().map(|cut| cut.height).fold(0.0, f64::max);
    let sign = spec.direction.sign();
    // Building up, a bench starts on its toe and ends on its crest.
    let (starts, ends) = match spec.direction {
        Direction::Up => (StringKind::Toe, StringKind::Crest),
        Direction::Down => (StringKind::Crest, StringKind::Toe),
    };
    let segments = |string: &DesignString| -> Vec<(DVec3, DVec3)> {
        let points = &string.points;
        let count = points.len();
        let pairs = if string.closed { count } else { count.saturating_sub(1) };
        (0..pairs).map(|index| (points[index], points[(index + 1) % count])).collect()
    };
    let rings: Vec<(StringKind, Vec<(DVec3, DVec3)>)> = design.strings.iter().filter(|string| string.closed).map(|string| (string.kind, segments(string))).collect();
    let road: Vec<(DVec3, DVec3)> = design.strings.iter().filter(|string| string.kind == StringKind::RampEdge).flat_map(segments).collect();
    let mut sums = vec![(0_u32, 0.0, 0.0, f64::NEG_INFINITY); bins.len()];
    let mut steepest_at: Vec<Option<DVec3>> = vec![None; bins.len()];
    let count = base.len();
    // The way the walls face, as the benches were cut to it.
    let first_walls = cuts.first().map(|cut| cut.walls(spec));
    let on_base = first_walls.as_ref().map(|walls| walls.on(base));
    for index in 0..count {
        let (a, b) = (base[index], base[(index + 1) % count]);
        let out = ring::outward(base, index) * sign;
        let samples = (a.distance(b) / INTER_RAMP_STEP).floor() as usize;
        for sample in 0..samples {
            let along = (sample as f64 + 0.5) / samples as f64;
            let point = a.lerp(b, along);
            let facing = on_base.as_ref().map_or(walls::facing(base, index), |on| on.facing(index as f64 + along));
            let far = point + out * 1.0e5;
            // As far as the line first meets the last ring: beyond, it is
            // across some other wall.
            let reach = (0..last.len())
                .filter_map(|edge| crossing(point, far, last[edge], last[(edge + 1) % last.len()]).map(|(hit, _)| hit.distance(point)))
                .fold(f64::INFINITY, f64::min);
            if !reach.is_finite() {
                continue;
            }
            let end = point + out * (reach + ring::SAME_POINT);
            // Every ring and ramp edge the line crosses, how far out: a ring
            // with its kind and RL, the ramp with none.
            let mut crossed: Vec<(f64, Option<(StringKind, f64)>)> = Vec::new();
            for (kind, ring) in &rings {
                for (c, d) in ring {
                    if let Some((hit, _)) = crossing(point, end, c.truncate(), d.truncate()) {
                        crossed.push((hit.distance(point), Some((*kind, c.z))));
                    }
                }
            }
            for (c, d) in &road {
                if let Some((hit, _)) = crossing(point, end, c.truncate(), d.truncate()) {
                    crossed.push((hit.distance(point), None));
                }
            }
            crossed.sort_by(|x, y| x.0.total_cmp(&y.0));
            // Each stack between ramp crossings: from the first ring a
            // bench starts on to the last a bench ends on.
            for stack in crossed.split(|(_, ring)| ring.is_none()) {
                let first = stack.iter().find(|(_, ring)| ring.is_some_and(|(kind, _)| kind == starts));
                let last = stack.iter().rev().find(|(_, ring)| ring.is_some_and(|(kind, _)| kind == ends));
                let (Some(&(near, Some((_, low)))), Some(&(far, Some((_, high))))) = (first, last) else {
                    continue;
                };
                let rise = (high - low) * sign;
                if far <= near || rise <= tallest * 1.01 {
                    continue;
                }
                // The benches in the stack, cut as the walls are half way up
                // it: each a face, and a berm before all but the first.
                let middle = point + out * (near + far) * 0.5;
                let mut across = 0.0;
                let mut first_bench = true;
                for (cut, bench) in cuts.iter().zip(&design.report) {
                    let (start, finish) = ((bench.elevation - low) * sign, (bench.elevation + sign * bench.height - low) * sign);
                    if start < -1.0e-6 || finish > rise + 1.0e-6 {
                        continue;
                    }
                    let (angle, berm) = cut.walls(spec).at(middle, facing);
                    across += face_run(cut.height, angle) + if first_bench { 0.0 } else { berm };
                    first_bench = false;
                }
                if across <= 0.0 {
                    continue;
                }
                let bin = walls::bin_of(&bins, walls::bearing_of(facing));
                let achieved = rise.atan2(far - near).to_degrees();
                let designed = rise.atan2(across).to_degrees();
                let sum = &mut sums[bin];
                if achieved > sum.3 {
                    steepest_at[bin] = Some(middle.extend((low + high) * 0.5));
                }
                *sum = (sum.0 + 1, sum.1 + designed, sum.2 + achieved, sum.3.max(achieved));
            }
        }
    }
    bins.iter()
        .zip(&targets)
        .zip(sums)
        .zip(steepest_at)
        .filter(|((_, (samples, ..)), _)| *samples > 0)
        .map(|(((bearing, target), (samples, designed, achieved, steepest)), steepest_at)| walls::WallReport {
            bearing: *bearing,
            samples,
            designed: designed / f64::from(samples),
            achieved: achieved / f64::from(samples),
            steepest,
            over_cut: (achieved - designed) / f64::from(samples),
            target: *target,
            steepest_at,
        })
        .collect()
}

/// A design as far as it has been built.
#[derive(Clone)]
struct State<'a> {
    builder: Builder<'a>,
    /// The ring the next bench starts from, at `elevation`.
    current: Vec<DVec2>,
    elevation: f64,
    levels: Vec<f64>,
    /// Where each ramp is, on a bench it is on.
    ramps: Vec<Option<RampCursor>>,
    benches: u32,
    volume: f64,
    report: Vec<BenchReport>,
}

impl State<'_> {
    /// How the next bench is cut, or `None` if the design is complete: the
    /// last bench before a stop RL is cut short to meet it.
    fn next_cut(&self) -> Option<Cut> {
        let spec = self.builder.spec;
        let cut = spec.cut(self.benches, self.elevation);
        match spec.stop {
            Stop::Benches(count) if self.benches >= count => None,
            Stop::Elevation(stop) => {
                let left = (stop - self.elevation) * self.builder.sign;
                (left > 1.0e-6).then(|| Cut {
                    height: left.min(cut.height),
                    ..cut
                })
            }
            _ => Some(cut),
        }
    }

    /// Each ramp on the next bench, cut as `cut` says: none where a ramp is
    /// not on it, and begun anew where it was not on the last. It leaves the
    /// pit on its last bench, building up (where that is known: not where
    /// the design stops on the ground), and on its first building down.
    fn ramps_for(&self, cut: &Cut) -> Vec<Option<RampOn>> {
        (0..self.builder.spec.ramps.len()).map(|index| self.ramp_for(index, cut)).collect()
    }

    /// [`Self::ramps_for`] for the ramp listed at `index`.
    fn ramp_for(&self, index: usize, cut: &Cut) -> Option<RampOn> {
        let spec = self.builder.spec;
        let ramp = spec.ramps.get(index)?;
        let sign = self.builder.sign;
        if !ramp.holds(self.elevation, sign) {
            return None;
        }
        let begins = self.ramps[index].is_none();
        let cursor = self.ramps[index].unwrap_or_else(|| RampCursor {
            at: ring::nearest(&self.current, ramp.start).0,
            // Building down walks the ramp from its top, the way it falls.
            direction: if ramp.climbs_clockwise == (spec.direction == Direction::Up) { -1.0 } else { 1.0 },
            handover: None,
            since_turn: 0,
            begun_at: self.elevation,
            asked_made: 0,
            due_since: None,
            heading: DVec2::ZERO,
        });
        let exits = match spec.direction {
            Direction::Up => {
                let last = match spec.stop {
                    Stop::Benches(count) => self.benches + 1 >= count,
                    Stop::Elevation(stop) => (stop - self.elevation) * sign <= cut.height + 1.0e-6,
                    Stop::Daylight => false,
                };
                last || !ramp.holds(self.elevation + sign * cut.height, sign)
            }
            Direction::Down => begins,
        };
        Some(RampOn { cursor, exits })
    }

    /// Take a bench built as `cut` says from where the design stands.
    fn advance(&mut self, bench: Bench, cut: Cut) {
        let height = cut.height;
        let (face_angle, berm) = cut.walls(self.builder.spec).range(&self.current);
        self.volume += frustum(ring::signed_area(&self.current), ring::signed_area(&bench.end), height);
        self.report.push(BenchReport {
            elevation: self.elevation,
            height,
            face_angle,
            berm,
            area: ring::signed_area(&bench.end),
            ramps: bench.ramps,
        });
        self.elevation += self.builder.sign * height;
        self.levels.push(self.elevation);
        self.benches += 1;
        self.ramps = bench.next_ramps;
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
        let cut = trial.next_cut()?;
        let snapshot = trial.clone();
        let bench = trial.builder.bench(&trial.current, trial.elevation, cut, trial.ramps_for(&cut), Some(after)).ok()?;
        after = bench.added?;
        trial.advance(bench, cut);
        remember(&mut rebuilt, snapshot, Some(after));
        let built = loop {
            if trial.benches > benches {
                break true;
            }
            let Some(cut) = trial.next_cut() else {
                break true;
            };
            let snapshot = trial.clone();
            match trial.builder.bench(&trial.current, trial.elevation, cut, trial.ramps_for(&cut), None) {
                Ok(bench) => {
                    let added = bench.added;
                    trial.advance(bench, cut);
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
    /// Whether its end rings are conditioned.
    condition: bool,
    strings: Vec<DesignString>,
    /// Each ramp's strings, joined up bench to bench.
    roads: Vec<Road>,
    ramp_length: f64,
    /// How far along the wall from where it was asked a ramp had to start
    /// to fit, at most.
    start_moved: f64,
    /// Every string built so far, for keeping the next bench clear of them.
    built: Built,
    /// The ramp last found no room on a bench.
    blocked: Option<usize>,
}

/// A ramp's strings: its edges, left and right of the way it is built, and
/// its centreline, joined up bench to bench.
#[derive(Clone, Default)]
struct Road {
    left: Vec<DVec3>,
    right: Vec<DVec3>,
    centreline: Vec<DVec3>,
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
    /// Where each ramp carries on from on the next bench.
    next_ramps: Vec<Option<RampCursor>>,
    ramps: Vec<RampReport>,
    /// What was added to its first stretch to leave it sound, if it has one.
    added: Option<Added>,
}

impl Laid {
    /// It as placed on its bench.
    fn placed(&self) -> Placed<'_> {
        Placed {
            stretch: &self.stretch,
            join: match (&self.turn, self.handover) {
                (Some(turn), _) => Join::TurnsBack(turn),
                (None, Some((handover, at))) => Join::CarriesOn { handover, at },
                (None, None) => Join::Begins,
            },
            on_start: self.on_start,
        }
    }
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
    /// Build one bench from the ring `from` at `elevation`, cut as `cut`
    /// says: its height, its face, and the berm it leaves first; with a
    /// stretch of each ramp in `ramps` that is on it.
    ///
    /// The ramps are laid one after another, those carrying on first, each
    /// against the bench's rings with those before it in place. Where a
    /// ramp's stretch would leave the bench unsound (its handover crowding a
    /// corner, say), it is laid with a flat before its climb, or failing
    /// every flat, a little longer, a hair flatter and never steeper, until
    /// it does not. Where no way of laying it does, the bench is not built,
    /// and the ramp is noted as the one with no room.
    ///
    /// With `after`, only additions after it are tried for the first ramp:
    /// the bench is being laid again because the one after it would not
    /// build.
    fn bench(&mut self, from: &[DVec2], elevation: f64, cut: Cut, ramps: Vec<Option<RampOn>>, after: Option<Added>) -> Result<Bench, End> {
        let spec = self.spec;
        let sign = self.sign;
        let height = cut.height;
        let walls = cut.walls(spec);
        let leaves_berm = !walls.no_berm();
        // A ramp beginning on a bench above the first starts on its berm, at
        // the toe of the face it climbs, as one beginning on the floor does:
        // it is laid along the bench's start ring, as from a bench with no
        // berm of its own, once the ramps carrying on have made that ring.
        // Laid from the crest below, the berm narrowing under it would tilt
        // the ring the road's far side is set off from.
        let on_start = |on: &RampOn| on.cursor.handover.is_none() && leaves_berm;
        let order: Vec<usize> = (0..ramps.len())
            .filter(|&index| ramps[index].is_some_and(|on| !on_start(&on)))
            .chain((0..ramps.len()).filter(|&index| ramps[index].is_some_and(|on| on_start(&on))))
            .collect();
        let mut laid: Vec<Laid> = Vec::new();
        let mut rings_now: Option<(Vec<DVec2>, Vec<DVec2>)> = None;
        for (place, &index) in order.iter().enumerate() {
            let (Some(on), Some(ramp_spec)) = (ramps[index], spec.ramps.get(index)) else {
                continue;
            };
            match self.lay_ramp(from, elevation, &walls, &laid, index, on, on_start(&on), if place == 0 { after } else { None }) {
                Ok((stretch, start_ring, end_ring)) => {
                    self.start_moved = self.start_moved.max(stretch.moved);
                    laid.push(stretch);
                    rings_now = Some((start_ring, end_ring));
                }
                Err(end) => {
                    self.blocked = Some(index);
                    let _ = ramp_spec;
                    return Err(end);
                }
            }
        }
        let (start_ring, end_ring) = match rings_now {
            Some(rings) => rings,
            None => rings(spec, from, &walls, &[]).and_then(|rings| self.clear_of_built(rings, &[], elevation, height))?,
        };
        let end_ring = if self.condition {
            self.conditioned(start_ring.as_slice(), end_ring, &laid, elevation, height)
        } else {
            end_ring
        };

        let end_elevation = elevation + sign * height;
        // With no berm the bench starts on the ring the last one ended on,
        // which is a string already.
        if leaves_berm {
            self.strings.push(level_string(spec, &start_ring, elevation, true));
        }
        self.strings.push(level_string(spec, &end_ring, end_elevation, false));
        for (ring, z) in [(&start_ring, elevation), (&end_ring, end_elevation)] {
            self.built
                .add((0..ring.len()).map(|index| (ring[index].extend(z), ring[(index + 1) % ring.len()].extend(z))));
        }

        // Where each ramp carries on from on the ring the next bench starts
        // from: where its near edge arrived.
        let mut next_ramps: Vec<Option<RampCursor>> = vec![None; ramps.len()];
        let mut reports: Vec<RampReport> = Vec::new();
        let mut by_ramp: Vec<&Laid> = laid.iter().collect();
        by_ramp.sort_by_key(|laid| laid.index);
        for laid in by_ramp {
            let Some(on) = ramps[laid.index] else {
                continue;
            };
            let (near, far) = self.lay_road(laid.index, &laid.stretch, laid.turn.as_ref(), elevation, height);
            reports.push(RampReport {
                ramp: laid.index,
                flat: laid.flat,
                lengthened: laid.lengthened,
                switchback: laid.turn.is_some(),
                turn_rise: laid.turn.as_ref().map_or(0.0, |turn| turn.lift),
                turn_at: laid
                    .turn
                    .as_ref()
                    .and_then(|turn| turn.centre.get(turn.centre.len() / 2))
                    .map(|point| point.extend(elevation)),
                // A turn put off from a bench before, for want of room there:
                // where it was asked, the RL it was asked at.
                turn_moved_from: match (on.cursor.due_since, spec.ramps.get(laid.index)) {
                    (Some(due), Some(ramp)) if laid.turn.is_some() && (due - elevation).abs() > 1.0e-6 => Some(asked_turn(ramp, &on.cursor, elevation, sign).unwrap_or(due)),
                    _ => None,
                },
                ..measure_road(&near, &far, height - laid.turn.as_ref().map_or(0.0, |turn| turn.lift))
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
            next_ramps[laid.index] = Some(RampCursor {
                at: ring::nearest(&end_ring, near_end.0).0,
                direction: laid.stretch.direction,
                handover: Some(far_end.0),
                since_turn: if laid.turn.is_some() { 1 } else { on.cursor.since_turn + 1 },
                begun_at: on.cursor.begun_at,
                // A turn made while one asked at an RL is owed is that one.
                asked_made: match spec.ramps.get(laid.index) {
                    Some(ramp) if laid.turn.is_some() && asked_reached(ramp, &on.cursor, elevation, sign) > on.cursor.asked_made => on.cursor.asked_made + 1,
                    _ => on.cursor.asked_made,
                },
                // A turn due here that did not fit is still due on the next.
                due_since: match (&laid.turn, spec.ramps.get(laid.index)) {
                    // Turned, but owing another asked here: due from here.
                    (Some(_), Some(ramp)) if asked_reached(ramp, &on.cursor, elevation, sign) > on.cursor.asked_made + 1 => Some(elevation),
                    (Some(_), _) => None,
                    (None, Some(ramp)) if on.cursor.handover.is_some() && turn_due(ramp, &on.cursor, elevation, sign) => on.cursor.due_since.or(Some(elevation)),
                    _ => on.cursor.due_since,
                },
                heading,
            });
        }
        let added = laid.first().map(|laid| Added {
            longer: laid.lengthened > 0.0,
            length: if laid.lengthened > 0.0 { laid.lengthened } else { laid.flat - laid.base_flat },
        });
        Ok(Bench {
            end: end_ring,
            next_ramps,
            ramps: reports,
            added,
        })
    }

    /// Lay ramp `index`'s stretch on a bench, with `laid` already in place:
    /// its stretch, and the bench's rings with it. Laid along the bench's
    /// start ring where `on_start` says, beginning on its berm.
    #[allow(clippy::too_many_arguments)]
    fn lay_ramp(
        &self,
        from: &[DVec2],
        elevation: f64,
        walls: &walls::Walls<'_>,
        laid: &[Laid],
        index: usize,
        on: RampOn,
        on_start: bool,
        after: Option<Added>,
    ) -> Result<(Laid, Vec<DVec2>, Vec<DVec2>), End> {
        let spec = self.spec;
        let sign = self.sign;
        let height = walls.height;
        let ramp_spec = &spec.ramps[index];
        // What the ramp's band, and its leaving the pit here, ask of this
        // bench's stretch: its width, and how far it runs level before and
        // after it climbs. Whatever is added to leave the bench sound comes
        // on top.
        let band = ramp_spec.band(elevation, sign);
        let exit = if on.exits { ramp_spec.exit_flat } else { 0.0 };
        let (before, flat_end) = match spec.direction {
            Direction::Up => (0.0, exit),
            Direction::Down => (exit, 0.0),
        };
        let width = band.and_then(|band| band.width).unwrap_or(ramp_spec.width);
        let base_flat = band.and_then(|band| band.flat).unwrap_or(0.0).max(ramp_spec.junction_flat) + before;
        // Laid along the start ring, the ring the ramps carrying on have
        // made, as from a bench with no berm.
        let start;
        let (along, lay_walls, cursor) = if on_start {
            let placed: Vec<Placed<'_>> = laid.iter().map(Laid::placed).collect();
            start = start_ring(spec, from, walls, &placed)?;
            let at = ring::nearest(&start, ring::point_at(from, on.cursor.at)).0;
            (start.as_slice(), walls::Walls { fixed_berm: Some(0.0), ..*walls }, RampCursor { at, ..on.cursor })
        } else {
            (from, *walls, on.cursor)
        };
        // Time for a switchback: tried first, and where it will not fit,
        // the ramp carries on round for another bench and tries again on
        // the next.
        let turn = match ramp_spec.layout {
            Layout::Switchback {
                inside_radius, widening, grade, ..
            } if cursor.handover.is_some() && turn_due(ramp_spec, &cursor, elevation, sign) => Some((inside_radius, widening, grade)),
            _ => None,
        };
        let mut last = End::OutOfRoom;
        for turn in [turn, None].into_iter().take(if turn.is_some() { 2 } else { 1 }) {
            // A flat before the climb first, which keeps the grade; only
            // where none works, a longer climb, a little flatter.
            let climb = height / ramp_spec.gradient;
            let start_drift = {
                let on = lay_walls.on(along);
                on.berm(cursor.at) + on.face(cursor.at)
            };
            let lay = |flat: f64, longer: f64| Lay {
                side: sign,
                drift: start_drift,
                // A stretch carrying on, whose end ring is spliced from its
                // far edge: a road's first stretch has its end ring set off
                // from the ring beside it.
                varies: !walls.alike() && cursor.handover.is_some(),
                far_start: cursor.handover.filter(|_| !walls.alike()),
                width,
                climb_length: climb + longer,
                flat: base_flat + flat,
                flat_end,
                // Carrying on from a road of another width, as far out as
                // that one's far edge arrived. Within the gap a handover
                // takes up, it is this width already.
                start_across: cursor
                    .handover
                    .map(|handover| handover.distance(ring::point_at(along, cursor.at)))
                    .filter(|across| (across - width).abs() > HANDOVER_GAP),
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
            let try_with = |lay: Lay| {
                self.attempt(
                    from,
                    along,
                    cursor,
                    lay,
                    turn,
                    walls,
                    &lay_walls,
                    elevation,
                    laid,
                    (index, on_start, base_flat),
                    ramp_spec.gradient,
                )
            };
            let flat = || match flats_after {
                Some(floor) => shortest(floor, &mut |flat| try_with(lay(flat, 0.0))),
                None => Err(End::OutOfRoom),
            };
            match flat().or_else(|_| shortest(longer_after, &mut |longer| try_with(lay(0.0, longer)))) {
                Ok(attempt) => return Ok(attempt),
                Err(end) => last = end,
            }
        }
        Err(last)
    }

    /// A bench's end ring conditioned (see [`ring::conditioned`]): moved
    /// only toward the waste side of its face, so the face only comes out
    /// flatter, and left alone within [`CONDITION_CLEAR`] of its road. The
    /// next bench is set off from it, so its berm is as wide as asked. Where
    /// the conditioned ring would not be as sound as the bench was, it stays
    /// as it was.
    fn conditioned(&self, start_ring: &[DVec2], end_ring: Vec<DVec2>, laid: &[Laid], elevation: f64, height: f64) -> Vec<DVec2> {
        let mut road: Vec<(DVec2, DVec2)> = Vec::new();
        for laid in laid {
            for edge in [&laid.stretch.edges.near, &laid.stretch.edges.far] {
                road.extend(edge.windows(2).map(|pair| (pair[0].0, pair[1].0)));
            }
            if let Some(turn) = &laid.turn {
                for chain in [&turn.inner, &turn.outer, &turn.pad] {
                    road.extend(chain.windows(2).map(|pair| (pair[0], pair[1])));
                }
            }
        }
        let keep = |point: DVec2| {
            road.iter()
                .any(|(a, b)| crate::model::kernel::project_onto_segment(point, *a, *b).0.distance(point) < CONDITION_CLEAR)
        };
        // Building up the waste side of a crest is out from the pit, and
        // building down, of a toe, in.
        let end = ring::conditioned(&end_ring, self.sign, &keep);
        let sound = (laid.is_empty() && !unsound(start_ring, &end, None))
            || laid.iter().all(|laid| {
                let stretch = &laid.stretch;
                !unsound(start_ring, &end, Some(stretch)) && !crowded_handover(&end, stretch) && arrives_on(&end, stretch)
            });
        let all: Vec<&Laid> = laid.iter().collect();
        if sound && let Ok((_, end)) = self.clear_of_built((start_ring.to_vec(), end), &all, elevation, height) {
            return end;
        }
        end_ring
    }

    /// One try at a ramp's stretch on a bench and the bench's rings with
    /// it and those `laid` before it: carried on from the last (or begun),
    /// or turned back on a pad `turn` the inside radius of its turn, along
    /// `along` (the ring the bench starts from, or its start ring).
    #[allow(clippy::too_many_arguments)]
    fn attempt(
        &self,
        from: &[DVec2],
        along: &[DVec2],
        cursor: RampCursor,
        lay: Lay,
        turn: Option<(f64, f64, f64)>,
        walls: &walls::Walls<'_>,
        lay_walls: &walls::Walls<'_>,
        elevation: f64,
        laid: &[Laid],
        (index, on_start, base_flat): (usize, bool, f64),
        gradient: f64,
    ) -> Result<(Laid, Vec<DVec2>, Vec<DVec2>), End> {
        let (stretch, turn, moved) = match (turn, cursor.handover) {
            (Some((radius, widening, grade)), Some(handover)) => {
                let (stretch, turn) = turn_back(along, cursor, handover, lay, (radius, widening, grade), (walls.height, gradient), lay_walls)?;
                (stretch, Some(turn), 0.0)
            }
            _ => {
                let (stretch, moved) = lay_stretch(along, cursor, lay, lay_walls)?;
                (stretch, None, moved)
            }
        };
        let height = walls.height;
        let candidate = Laid {
            index,
            stretch,
            turn,
            handover: cursor.handover.map(|handover| (handover, cursor.at)),
            on_start,
            base_flat,
            moved,
            flat: lay.flat,
            lengthened: lay.climb_length - height / gradient,
        };
        let (start_ring, end_ring) = {
            let placed: Vec<Placed<'_>> = laid.iter().chain(std::iter::once(&candidate)).map(Laid::placed).collect();
            let rings = rings(self.spec, from, walls, &placed)?;
            let all: Vec<&Laid> = laid.iter().chain(std::iter::once(&candidate)).collect();
            self.clear_of_built(rings, &all, elevation, height)?
        };
        Ok((candidate, start_ring, end_ring))
    }

    /// A bench's rings, if they and its stretch keep clear of everything
    /// built before them: no string of the design may cross another at a
    /// different height, so a bench that would is not built.
    fn clear_of_built(&self, rings: (Vec<DVec2>, Vec<DVec2>), laid: &[&Laid], elevation: f64, height: f64) -> Result<(Vec<DVec2>, Vec<DVec2>), End> {
        let end_elevation = elevation + self.sign * height;
        let mut candidate: Vec<(DVec3, DVec3)> = Vec::new();
        for (ring, z) in [(&rings.0, elevation), (&rings.1, end_elevation)] {
            candidate.extend((0..ring.len()).map(|index| (ring[index].extend(z), ring[(index + 1) % ring.len()].extend(z))));
        }
        for laid in laid {
            let stretch = &laid.stretch;
            let lift = laid.turn.as_ref().map_or(0.0, |turn| turn.lift);
            for edge in [&stretch.edges.near, &stretch.edges.far] {
                let points: Vec<DVec3> = edge
                    .iter()
                    .map(|(point, source)| point.extend(stretch_height(elevation, self.sign, height, lift, stretch.climbed_at(*source).unwrap_or(1.0))))
                    .collect();
                candidate.extend(points.windows(2).map(|pair| (pair[0], pair[1])));
            }
            if let Some(turn) = &laid.turn {
                for edge in [&turn.inner, &turn.outer] {
                    let points = turn.at_height(edge, elevation, self.sign);
                    candidate.extend(points.windows(2).map(|pair| (pair[0], pair[1])));
                }
            }
        }
        if self.built.crossed_by(&candidate) { Err(End::OutOfRoom) } else { Ok(rings) }
    }

    /// Add a stretch's edges and centreline to the ramp's strings, and give
    /// back its edges as laid, near then far.
    fn lay_road(&mut self, index: usize, stretch: &Stretch, turn: Option<&Turn>, elevation: f64, height: f64) -> (Vec<DVec3>, Vec<DVec3>) {
        let road = &mut self.roads[index];
        let mut ramp_length = 0.0;
        let lift = turn.map_or(0.0, |turn| turn.lift);
        if let Some(turn) = turn {
            // Round the turn, level on its pad or climbing: each edge of the
            // road arriving runs on round the turn, the near one the outside
            // of it.
            let level = |points: &[DVec2]| -> Vec<DVec3> { turn.at_height(points, elevation, self.sign) };
            let (outer, inner) = (level(&turn.outer), level(&turn.inner));
            let near_on_left = road.left.last().zip(outer.first()).is_some_and(|(left, start)| left.distance(*start) <= HANDOVER_GAP);
            let (left, right) = if near_on_left { (outer, inner) } else { (inner, outer) };
            for edge in [&left, &right] {
                self.built.add(edge.windows(2).map(|pair| (pair[0], pair[1])));
            }
            join(&mut road.left, left);
            join(&mut road.right, right);
            let centre = level(&turn.centre);
            ramp_length += turn.centre.windows(2).map(|pair| pair[0].distance(pair[1])).sum::<f64>();
            join(&mut road.centreline, centre);
        }
        let sign = self.sign;
        let at_height = |chain: &[(DVec2, f64)]| -> Vec<DVec3> {
            chain
                .iter()
                .map(|(point, source)| point.extend(stretch_height(elevation, sign, height, lift, stretch.climbed_at(*source).unwrap_or(1.0))))
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
        join(&mut road.left, left);
        join(&mut road.right, right);
        // The centreline at each station, half way between the edges.
        let centre: Vec<DVec3> = stretch
            .stations
            .iter()
            .map(|(parameter, climbed)| {
                let near = point_by_source(&stretch.edges.near, *parameter, stretch.count);
                let far = point_by_source(&stretch.edges.far, *parameter, stretch.count);
                let middle = near.lerp(far, 0.5);
                middle.extend(stretch_height(elevation, sign, height, lift, *climbed))
            })
            .collect();
        ramp_length += centre.windows(2).map(|pair| pair[0].truncate().distance(pair[1].truncate())).sum::<f64>();
        join(&mut road.centreline, centre);
        self.ramp_length += ramp_length;
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
    let mut steepest_at = None;
    for edge in [near, far] {
        let Some(&first) = edge.first() else {
            continue;
        };
        let (mut anchor, mut run) = (first, 0.0);
        for pair in edge.windows(2) {
            run += pair[0].truncate().distance(pair[1].truncate());
            if run >= 1.0 {
                let slope = (pair[1].z - anchor.z).abs() / run;
                if slope > steepest {
                    steepest_at = Some(anchor.lerp(pair[1], 0.5));
                }
                steepest = steepest.max(slope);
                anchor = pair[1];
                run = 0.0;
            }
        }
    }
    let mut narrowest = f64::INFINITY;
    let mut narrowest_at = None;
    for point in near {
        let nearest = far
            .windows(2)
            .filter(|pair| point.z >= pair[0].z.min(pair[1].z) - SAME_HEIGHT && point.z <= pair[0].z.max(pair[1].z) + SAME_HEIGHT)
            .map(|pair| crate::model::kernel::project_onto_segment(point.truncate(), pair[0].truncate(), pair[1].truncate()).0)
            .map(|across| (across.distance(point.truncate()), across))
            .fold((f64::INFINITY, DVec2::ZERO), |best, next| if next.0 < best.0 { next } else { best });
        if nearest.0 < narrowest {
            narrowest_at = Some(point.truncate().lerp(nearest.1, 0.5).extend(point.z));
        }
        narrowest = narrowest.min(nearest.0);
    }
    RampReport {
        ramp: 0,
        grade,
        steepest,
        narrowest: if narrowest.is_finite() { narrowest } else { 0.0 },
        steepest_at,
        narrowest_at,
        middle: checks::half_way(near),
        flat: 0.0,
        lengthened: 0.0,
        switchback: false,
        turn_moved_from: None,
        turn_at: None,
        turn_rise: 0.0,
    }
}

/// Whether a ramp laid as `ramp` turns back on the bench starting at
/// `elevation` (built the way `sign` says): every so many benches, once for
/// each RL it asks that the ramp has reached, and on any bench after one
/// where a turn was due and would not fit.
fn turn_due(ramp: &RampSpec, cursor: &RampCursor, elevation: f64, sign: f64) -> bool {
    let Layout::Switchback { every, .. } = ramp.layout else {
        return false;
    };
    cursor.due_since.is_some() || (every > 0 && cursor.since_turn >= every) || asked_reached(ramp, cursor, elevation, sign) > cursor.asked_made
}

/// The RL asked for the turn a ramp owes on the bench starting at
/// `elevation`, if it owes one asked at an RL: the earliest it has not yet
/// made, the way the design is built.
fn asked_turn(ramp: &RampSpec, cursor: &RampCursor, elevation: f64, sign: f64) -> Option<f64> {
    let mut reached: Vec<f64> = ramp
        .turns_at
        .iter()
        .copied()
        .filter(|rl| (rl - cursor.begun_at) * sign > 1.0e-6 && (elevation - rl) * sign >= -1.0e-6)
        .collect();
    reached.sort_by(|a, b| (a * sign).total_cmp(&(b * sign)));
    reached.get(cursor.asked_made as usize).copied()
}

/// How many of the RLs a ramp asks to turn at it has reached by the bench
/// starting at `elevation`, since it began: each its own turn.
fn asked_reached(ramp: &RampSpec, cursor: &RampCursor, elevation: f64, sign: f64) -> u32 {
    let reached = ramp
        .turns_at
        .iter()
        .filter(|rl| (**rl - cursor.begun_at) * sign > 1.0e-6 && (elevation - **rl) * sign >= -1.0e-6)
        .count();
    u32::try_from(reached).unwrap_or(u32::MAX)
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

/// How far the road drifts across over a bench at each point of the ring
/// it leaves: a berm and a face, as wide there as the walls are cut.
fn drift_along<'a>(from: &'a [DVec2], walls: &'a walls::Walls<'_>) -> impl Fn(f64) -> f64 + 'a {
    let on = walls.on(from);
    move |parameter| on.berm(parameter) + on.face(parameter)
}

/// Lay a bench's stretch of ramp from `cursor` along `from`, and how far
/// along the wall it had to start from where it was asked to fit: a ramp
/// that begins off a tip too sharp to stand its width beside starts a
/// little further along.
fn lay_stretch(from: &[DVec2], cursor: RampCursor, lay: Lay, walls: &walls::Walls<'_>) -> Result<(Stretch, f64), End> {
    let drift = drift_along(from, walls);
    let reach = |climbed: f64, parameter: f64| climbed * drift(parameter);
    if let Some(handover) = cursor.handover {
        let mut stretch = road::lay(from, cursor.at, cursor.direction, lay, &reach).map_err(|_| End::RampTooLong)?;
        // It starts exactly where the last one arrived: laid off the ring's
        // corner there, or a little wider or narrower square across than
        // the last where the benches are cut differently, its far edge can
        // start a little to one side. The step across to it is level, set
        // off from the same point of the ring as the edge's start, so it
        // takes nothing from the climb: moving the start itself would
        // shorten the first piece and steepen it.
        if let Some(&(first, source)) = stretch.edges.far.first()
            && first.distance(handover) <= HANDOVER_GAP
            && first.distance(handover) > MEETS
        {
            stretch.edges.far.insert(0, (handover, source));
        }
        return Ok((stretch, 0.0));
    }
    let arcs = ring::arcs(from);
    let around = arcs[from.len()];
    let picked = ring::arc_at(&arcs, cursor.at);
    let mut moved = 0.0;
    loop {
        let at = ring::parameter_at(&arcs, picked + cursor.direction * moved);
        let stretch = road::lay(from, at, cursor.direction, lay, &reach).map_err(|_| End::RampTooLong)?;
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
#[allow(clippy::too_many_arguments)]
fn turn_back(
    from: &[DVec2],
    cursor: RampCursor,
    handover: DVec2,
    lay: Lay,
    (radius, widening, grade): (f64, f64, f64),
    (height, gradient): (f64, f64),
    walls: &walls::Walls<'_>,
) -> Result<(Stretch, Turn), End> {
    let near_end = ring::point_at(from, cursor.at);
    let across = (handover - near_end).try_normalize().ok_or(End::OutOfRoom)?;
    // Which way the road was heading as it arrived, square to the step
    // across it.
    let along = (cursor.heading - across * cursor.heading.dot(across)).try_normalize().ok_or(End::OutOfRoom)?;
    let reach = 2.0 * radius;
    let back_at = ring::nearest(from, handover).0;
    let drift = drift_along(from, walls);
    // Climbing round the turn, its inside edge at `grade` over half a
    // circle, and no more than most of the bench: the stretch after it
    // climbs what is left. Laid, the inside edge is measured, and where it
    // is shorter the turn climbs less, never steeper than asked.
    let mut lift = if grade > 0.0 {
        (std::f64::consts::PI * radius * grade).min(height * MOST_TURN_LIFT)
    } else {
        0.0
    };
    for _ in 0..TURN_ROUNDS {
        // Turned back across its pad, it starts its own width out.
        let lay = Lay {
            start_across: None,
            climb_length: lay.climb_length - lift / gradient,
            ..lay
        };
        let stretch = road::lay(from, back_at, -cursor.direction, lay, &|climbed, parameter| {
            climbed * drift(parameter) + reach * (1.0 - climbed)
        })
        .map_err(|_| End::RampTooLong)?;
        let (Some(leave_near), Some(leave_far)) = (stretch.edges.near.first(), stretch.edges.far.first()) else {
            return Err(End::OutOfRoom);
        };
        let centre = handover + across * radius;
        // Wider round the middle of the turn, on the outside.
        let inner = half_turn(handover, leave_near.0, centre, along, 0.0);
        let outer = half_turn(near_end, leave_far.0, centre, along, widening);
        let middle = half_turn(near_end.lerp(handover, 0.5), leave_near.0.lerp(leave_far.0, 0.5), centre, along, widening * 0.5);
        let inside: f64 = inner.windows(2).map(|pair| pair[0].distance(pair[1])).sum();
        if grade > 0.0 && inside * grade < lift - 1.0e-6 {
            lift = inside * grade;
            continue;
        }
        let mut pad = vec![handover, leave_near.0];
        pad.extend(outer.iter().rev().copied());
        pad.dedup_by(|a, b| a.distance(*b) <= ring::SAME_POINT);
        return Ok((
            stretch,
            Turn {
                inner,
                outer,
                centre: middle,
                pad,
                lift,
            },
        ));
    }
    Err(End::OutOfRoom)
}

/// The most of a bench a turn climbs: the stretch after it climbs the rest.
const MOST_TURN_LIFT: f64 = 0.75;

/// How many times a climbing turn is laid, its climb lowered to what its
/// inside edge allows.
const TURN_ROUNDS: usize = 3;

/// How much of a turn's widening it has `t` of the way round its outside:
/// none until it has left the crest it set out along (where widening would
/// put the road out over the bench below), then easing out and back to
/// none where it rejoins the wall.
fn widened(t: f64) -> f64 {
    if t <= WIDEN_FROM {
        0.0
    } else {
        (std::f64::consts::PI * (t - WIDEN_FROM) / (1.0 - WIDEN_FROM)).sin()
    }
}

/// How far round a turn's outside its widening starts.
const WIDEN_FROM: f64 = 0.25;

/// Half a turn round `centre` from `from` to `to`, bulging the way `ahead`
/// points: both ends included, and between them points whose chords touch
/// the circle (its radius going evenly from one end's to the other's, and
/// `bulge` more round the middle as [`widened`] says), so the road is
/// nowhere narrower round it than at its ends.
fn half_turn(from: DVec2, to: DVec2, centre: DVec2, ahead: DVec2, bulge: f64) -> Vec<DVec2> {
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
        let radius = (start_radius + (end_radius - start_radius) * t + bulge * widened(t)) / half_step.cos();
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

/// A stretch of ramp placed on a bench: how it joins what came before it,
/// and whether it was laid along the bench's start ring (a ramp beginning
/// on a bench above the first, on its berm) rather than along the ring the
/// bench starts from.
#[derive(Clone, Copy)]
struct Placed<'a> {
    stretch: &'a Stretch,
    join: Join<'a>,
    on_start: bool,
}

/// The ring a bench starts on: the ring it starts from, `from`, set off
/// across the berm. Under each stretch of road laid along `from`, the berm
/// narrows to nothing, the road taking its place; where a stretch carries
/// on from the one before, the ring steps across the road to it, and that
/// step is road, not set off at all. Round a switchback's pad, level with
/// the bench's start, the ring goes round the pad.
fn start_ring(spec: &Spec, from: &[DVec2], walls: &walls::Walls<'_>, placed: &[Placed<'_>]) -> Result<Vec<DVec2>, End> {
    let sign = spec.direction.sign();
    let count = from.len();
    let along_from: Vec<&Placed<'_>> = placed.iter().filter(|placed| !placed.on_start).collect();
    let start_ring = if walls.no_berm() {
        from.to_vec()
    } else {
        // A point at each of the stretches' stations, where their edges have
        // theirs: the berm narrows under each road point for point with it,
        // and so stays the road's own side of its near edge. And points
        // enough near a slope region that its blend is followed.
        let handovers: Vec<(DVec2, DVec2)> = along_from
            .iter()
            .filter_map(|placed| match placed.join {
                Join::CarriesOn { handover, .. } => Some((handover, ring::point_at(from, placed.stretch.from))),
                _ => None,
            })
            .collect();
        // The step across the road where a last stretch arrived is road,
        // set off by nothing: it is found as one edge, so it is never split.
        let steps: Vec<usize> = handovers
            .iter()
            .filter_map(|(pushed, start)| {
                (0..count).find(|&index| {
                    let (a, b) = (from[index], from[(index + 1) % count]);
                    (a.distance(*pushed) <= MEETS && b.distance(*start) <= MEETS) || (a.distance(*start) <= MEETS && b.distance(*pushed) <= MEETS)
                })
            })
            .collect();
        let mut cuts: Vec<f64> = along_from
            .iter()
            .flat_map(|placed| placed.stretch.stations.iter().map(|(parameter, _)| *parameter))
            .collect();
        // A turn climbing round leaves no berm beside it: it starts on the
        // berm's level and rises off it, as a road does. The ring is cut
        // where each point of it stands square to the ring, so the berm
        // goes beside it alone.
        let climbing: Vec<&Turn> = along_from
            .iter()
            .filter_map(|placed| match placed.join {
                Join::TurnsBack(turn) if turn.lift > 0.0 => Some(turn),
                _ => None,
            })
            .collect();
        for turn in &climbing {
            cuts.extend(turn.inner.iter().chain(&turn.outer).map(|point| ring::nearest(from, *point).0));
        }
        cuts.extend(walls.splits(from).into_iter().filter(|parameter| !steps.contains(&(parameter.floor() as usize))));
        let (split, at) = ring::with_points_at(from, &cuts);
        let facings = walls.facings(&split);
        let distances: Vec<EdgeOffset> = (0..split.len())
            .map(|index| {
                let next = (index + 1) % split.len();
                let is = |a: DVec2, b: DVec2| {
                    (split[index].distance(a) <= MEETS && split[next].distance(b) <= MEETS) || (split[index].distance(b) <= MEETS && split[next].distance(a) <= MEETS)
                };
                if handovers.iter().any(|(pushed, start)| is(*pushed, *start)) {
                    return EdgeOffset::even(0.0);
                }
                if !climbing.is_empty() {
                    let middle = split[index].lerp(split[next], 0.5);
                    let clear = walls.berm_at(middle, facings[index]) + walls.face_at(middle, facings[index]);
                    if climbing.iter().any(|turn| beside_turn(turn, middle, clear)) {
                        return EdgeOffset::even(0.0);
                    }
                }
                let middle = middle_parameter(at[index], at[next], count);
                // Each point takes the way its wall faces as
                // [`walls::Walls::offsets`] has it.
                let start = walls.berm_at(split[index], facings[index]);
                let end = walls.berm_at(split[next], facings[next]);
                match along_from.iter().find(|placed| placed.stretch.climbed_at(middle).is_some()) {
                    Some(placed) => EdgeOffset {
                        start: sign * start * placed.stretch.climbed_at(at[index]).unwrap_or(0.0),
                        end: sign * end * placed.stretch.climbed_at(at[next]).unwrap_or(1.0),
                    },
                    None => EdgeOffset {
                        start: sign * start,
                        end: sign * end,
                    },
                }
            })
            .collect();
        let mut start_ring = walls.unfolded(walls.offset(&split, &distances).into_iter().next().ok_or(End::OutOfRoom)?);
        for placed in &along_from {
            if let Join::CarriesOn { .. } = placed.join {
                start_ring = along_far_edge(start_ring, placed.stretch);
            }
        }
        start_ring
    };
    // A switchback's pad is level with the bench's start: the ring goes
    // round it.
    let mut start_ring = start_ring;
    for placed in &along_from {
        if let Join::TurnsBack(turn) = placed.join
            && turn.lift == 0.0
        {
            start_ring = with_pad(&start_ring, &turn.pad).ok_or(End::OutOfRoom)?;
        }
    }
    // Near a slope region, the start ring with points enough to follow its
    // blend across the face.
    Ok(match walls.splits(&start_ring) {
        splits if splits.is_empty() => start_ring,
        splits => ring::with_points_at(&start_ring, &splits).0,
    })
}

/// The two rings of a bench: the ring it starts on (see [`start_ring`]), and
/// the ring it ends on, the start ring set off across the face, beside each
/// stretch of road where the face above it comes to. Each stretch's end
/// ring is worked out on its own, and where there are several they are
/// spliced into one, each changing the plain ring only round its own road.
/// `None` where the bench is not a sound piece of design.
fn rings(spec: &Spec, from: &[DVec2], walls: &walls::Walls<'_>, placed: &[Placed<'_>]) -> Result<(Vec<DVec2>, Vec<DVec2>), End> {
    let sign = spec.direction.sign();
    let start_ring = start_ring(spec, from, walls, placed)?;
    let plain = || {
        let distances = walls.offsets(&start_ring, sign, |walls, point, facing| walls.face_at(point, facing));
        walls
            .offset(&start_ring, &distances)
            .into_iter()
            .next()
            .map(|ring| walls.unfolded(ring))
            .ok_or(End::OutOfRoom)
    };
    let end_ring = match placed {
        [] => plain()?,
        [only] => end_beside(&start_ring, &plain, from, walls, sign, only)?,
        several => {
            let plain_ring = plain()?;
            let ends: Vec<Vec<DVec2>> = several
                .iter()
                .map(|placed| end_beside(&start_ring, &|| Ok(plain_ring.clone()), from, walls, sign, placed))
                .collect::<Result<_, _>>()?;
            merged(&plain_ring, &ends).ok_or(End::OutOfRoom)?
        }
    };
    // The handovers the next stretches start from stay as they are.
    let keep: Vec<DVec2> = placed
        .iter()
        .flat_map(|placed| [placed.stretch.edges.near.last().map(|point| point.0), placed.stretch.edges.far.last().map(|point| point.0)])
        .flatten()
        .collect();
    let end_ring = ring::filleted(&end_ring, walls.wall_radius, sign, &keep);
    let sound = (placed.is_empty() && !unsound(&start_ring, &end_ring, None))
        || placed.iter().all(|placed| {
            let stretch = placed.stretch;
            !unsound(&start_ring, &end_ring, Some(stretch))
                && !crowded_handover(&end_ring, stretch)
                && arrives_on(&end_ring, stretch)
                && carries_on(from, stretch, placed.join)
                && !pinched(stretch, stretch.width)
        });
    // Two roads on one bench keep apart: one may not cross the other, nor
    // come within a road's width of it.
    let apart = placed.iter().enumerate().all(|(index, first)| {
        placed[index + 1..].iter().all(|second| {
            let edges = |stretch: &Stretch| -> Vec<(DVec2, DVec2)> {
                [&stretch.edges.near, &stretch.edges.far]
                    .into_iter()
                    .flat_map(|edge| edge.windows(2).map(|pair| (pair[0].0, pair[1].0)))
                    .collect()
            };
            let (a, b) = (edges(first.stretch), edges(second.stretch));
            let gap = first.stretch.width.min(second.stretch.width);
            !crosses(&a, &b) && b.iter().all(|(c, d)| a.iter().all(|(e, f)| segment_gap(*c, *d, *e, *f) >= gap))
        })
    });
    if !sound || !apart {
        return Err(End::OutOfRoom);
    }
    Ok((start_ring, end_ring))
}

/// The ring a bench ends on beside one stretch of road, alone: set off
/// beside a new road from the start ring, or spliced into the plain ring
/// (`plain`) along one carrying on.
fn end_beside(
    start_ring: &[DVec2],
    plain: &dyn Fn() -> Result<Vec<DVec2>, End>,
    from: &[DVec2],
    walls: &walls::Walls<'_>,
    sign: f64,
    placed: &Placed<'_>,
) -> Result<Vec<DVec2>, End> {
    match placed.join {
        Join::Begins => beside_new_ramp(start_ring, placed.stretch, walls, sign).ok_or(End::OutOfRoom),
        Join::TurnsBack(turn) if turn.lift > 0.0 => spliced(&plain()?, placed.stretch, from, walls, sign, Some(turn)).ok_or(End::OutOfRoom),
        _ => spliced(&plain()?, placed.stretch, from, walls, sign, None).ok_or(End::OutOfRoom),
    }
}

/// Several end rings, each the plain ring changed round one road, spliced
/// into one: each one's run off the plain ring put in place of the same
/// stretch of the others. `None` where a ring leaves the plain ring in more
/// than one run, or where two runs overlap.
fn merged(plain: &[DVec2], rings: &[Vec<DVec2>]) -> Option<Vec<DVec2>> {
    let mut merged = rings.first()?.clone();
    for ring in &rings[1..] {
        let count = ring.len();
        let off: Vec<bool> = ring.iter().map(|point| ring::nearest(plain, *point).1 > MERGE_OFF).collect();
        let starts: Vec<usize> = (0..count).filter(|&index| off[index] && !off[(index + count - 1) % count]).collect();
        let &[first] = starts.as_slice() else {
            if starts.is_empty() {
                continue;
            }
            return None;
        };
        // The run off the plain ring, with the points on it either side
        // where it leaves and rejoins.
        let leave = (first + count - 1) % count;
        let mut run = vec![ring[leave]];
        let mut index = first;
        while off[index] {
            run.push(ring[index]);
            index = (index + 1) % count;
        }
        run.push(ring[index]);
        let (from, to) = (run[0], run[run.len() - 1]);
        // Where it leaves and rejoins the ring merged so far, which is the
        // plain ring there unless two runs overlap.
        let (at_from, off_from) = ring::nearest(&merged, from);
        let (at_to, off_to) = ring::nearest(&merged, to);
        if off_from > MERGE_OFF || off_to > MERGE_OFF {
            return None;
        }
        // The run, then the merged ring on round from where it rejoins to
        // where it leaves.
        let total = merged.len();
        let mut points = run;
        let mut index = (at_to.floor() as usize + 1) % total;
        let span = (at_from - at_to).rem_euclid(total as f64);
        let mut walked = (index as f64 - at_to).rem_euclid(total as f64);
        while walked < span {
            points.push(merged[index]);
            index = (index + 1) % total;
            walked += 1.0;
        }
        merged = ring::cleaned(points);
        if merged.len() < 3 || ring::signed_area(&merged) <= 0.0 {
            return None;
        }
    }
    Some(merged)
}

/// How far a ring may stand off the plain ring and still be on it, in
/// splicing several roads' rings into one.
const MERGE_OFF: f64 = 1.0e-3;

/// How near segment `a`-`b` comes to segment `c`-`d`.
fn segment_gap(a: DVec2, b: DVec2, c: DVec2, d: DVec2) -> f64 {
    if crossing(a, b, c, d).is_some() {
        return 0.0;
    }
    [(a, c, d), (b, c, d), (c, a, b), (d, a, b)]
        .into_iter()
        .map(|(point, from, to)| crate::model::kernel::project_onto_segment(point, from, to).0.distance(point))
        .fold(f64::INFINITY, f64::min)
}

/// The ring a bench ends on, where a stretch of ramp crosses it: the start
/// ring set off across the face (`plain`), with the stretch's length of it
/// replaced by the road's far edge set off by what is left of the face above
/// it, which is less the higher the road has climbed. Where the road
/// arrives, the ring crosses it straight to where its near edge arrived;
/// where it carries on from the stretch before, the far edge's offset runs
/// back until it meets the wall behind, the end of the berm the road takes
/// over, benched back like any other wall.
fn spliced(plain: &[DVec2], stretch: &Stretch, from: &[DVec2], walls: &walls::Walls<'_>, sign: f64, turn: Option<&Turn>) -> Option<Vec<DVec2>> {
    let direction = stretch.direction;
    let mut far: Vec<DVec2> = stretch.edges.far.iter().map(|point| point.0).collect();
    // The face above each point of the far edge is cut as the wall it was
    // set off from.
    let on_from = walls.on(from);
    let mut left_of_face: Vec<f64> = match turn {
        None => stretch
            .edges
            .far
            .iter()
            .map(|(_, source)| on_from.face(*source) * (1.0 - stretch.climbed_at(*source).unwrap_or(1.0)))
            .collect(),
        // After a turn that climbed, the road started that much up the face.
        Some(turn) => {
            let height = walls.height;
            stretch
                .edges
                .far
                .iter()
                .map(|(_, source)| {
                    let up = (turn.lift + (height - turn.lift) * stretch.climbed_at(*source).unwrap_or(1.0)) / height;
                    on_from.face(*source) * (1.0 - up)
                })
                .collect()
        }
    };
    // Round a turn that climbed, the face above its outside edge comes to
    // where the crest goes round it, shorter the further round it has
    // climbed: the outside edge first, then the far edge on from it.
    if let Some(turn) = turn {
        let face = stretch.edges.far.first().map_or(0.0, |(_, source)| on_from.face(*source));
        let raised = turn.at_height(&turn.outer, 0.0, 1.0);
        let count = raised.len().saturating_sub(1);
        let mut chain: Vec<DVec2> = turn.outer[..count].to_vec();
        let mut left: Vec<f64> = raised[..count].iter().map(|point| face * (1.0 - point.z / walls.height)).collect();
        chain.append(&mut far);
        left.append(&mut left_of_face);
        far = chain;
        left_of_face = left;
    }
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
    // The ring passes through where the road's near edge arrived: on the
    // plain ring where the walls are alike, and a hair from it where they
    // vary, the near edge's end being set off from the ring the bench
    // starts on and the plain ring from its berm's.
    let arrival = if walls.alike() {
        ring::point_at(plain, ring::parameter_at(&arcs, end_arc))
    } else {
        near_end
    };
    let mut points: Vec<DVec2> = vec![arrival];
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
fn beside_new_ramp(start_ring: &[DVec2], stretch: &Stretch, walls: &walls::Walls<'_>, sign: f64) -> Option<Vec<DVec2>> {
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
        let facings = walls.facings(&split);
        let distances: Vec<EdgeOffset> = (0..split.len())
            .map(|index| {
                let next = (index + 1) % split.len();
                let middle_arc = ring::arc_at(&arcs, middle_parameter(at[index], at[next], count));
                let along = ((middle_arc - start_arc) * direction).rem_euclid(around);
                let road = if along < run { width } else { 0.0 };
                EdgeOffset {
                    start: sign * (walls.face_at(split[index], facings[index]) + road),
                    end: sign * (walls.face_at(split[next], facings[next]) + road),
                }
            })
            .collect();
        walls.offset(&split, &distances).into_iter().next().map(|ring| walls.unfolded(ring))
    };
    // The head wall rises behind where the road starts, a face's run behind
    // its far edge at least: square over it, the wall would stand vertical.
    // Where the road starts at a corner, behind it is round the corner, and
    // the step goes further back until the road's start stands clear.
    // The face as it is cut where the road begins.
    let face = walls.on(start_ring).face(ring::nearest(start_ring, near_start).0);
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

/// Whether `point` stands within `clear` of a turn's edges.
fn beside_turn(turn: &Turn, point: DVec2, clear: f64) -> bool {
    [&turn.inner, &turn.outer].into_iter().any(|edge| {
        edge.windows(2)
            .any(|pair| crate::model::kernel::project_onto_segment(point, pair[0], pair[1]).0.distance(point) < clear)
    })
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
