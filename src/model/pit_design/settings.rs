//! What a pit design is designed with: everything the engine needs beyond
//! the string it starts from, in the units people give it.

use glam::DVec2;
use serde::{Deserialize, Serialize};

use super::{BenchRules, Direction, Kind, Layout, RampBand, RampSpec, Spec, Stop};
use crate::model::triangulation::TriangulationId;

/// Where a design stops, as its settings say.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum StopMode {
    Benches,
    Elevation,
    Daylight,
}

/// Everything a pit design sets. Two designs from the same string and equal
/// settings are the same design.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct PitDesignSettings {
    /// A pit cut into the ground, or a dump or stockpile built on it.
    pub(crate) kind: Kind,
    pub(crate) direction: Direction,
    pub(crate) bench_height: f64,
    /// Degrees from horizontal.
    pub(crate) face_angle: f64,
    pub(crate) berm_width: f64,
    /// How the benches vary from one to the next: a cycle of heights, catch
    /// berms, and bands of RL cut otherwise.
    pub(crate) bench_rules: BenchRules,
    pub(crate) wall_radius: f64,
    /// The ramps, each laid on the benches it runs on: none for a design
    /// with no ramp.
    pub(crate) ramps: Vec<RampSettings>,
    pub(crate) stop: StopMode,
    pub(crate) benches: u32,
    pub(crate) stop_elevation: f64,
    /// The ground a design built up daylights against.
    pub(crate) topography: Option<TriangulationId>,
}

/// One ramp of a pit design, as its settings say.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct RampSettings {
    pub(crate) width: f64,
    /// Rise over run, as a percentage.
    pub(crate) gradient: f64,
    /// Where it leaves the string, in plan; the string's first point if
    /// none has been picked.
    pub(crate) start: Option<DVec2>,
    pub(crate) climbs_clockwise: bool,
    pub(crate) switchback: bool,
    /// Benches climbed between turns: none, turning only at `turns_at`, at
    /// 0.
    pub(crate) switchback_every: u32,
    /// RLs it turns back at, as well as every so many benches.
    pub(crate) turns_at: Vec<f64>,
    pub(crate) inside_radius: f64,
    /// How much wider than its width the road is round the middle of a turn.
    pub(crate) turn_widening: f64,
    /// The RLs it begins and ends at, the way the design is built; the
    /// whole pit where none is given.
    pub(crate) from: Option<f64>,
    pub(crate) to: Option<f64>,
    /// Bands of RL whose stretches of road are laid otherwise: narrower, or
    /// with a flat.
    pub(crate) bands: Vec<RampBand>,
    /// How far it runs level where it leaves the pit.
    pub(crate) exit_flat: f64,
    /// How far each stretch runs level where it leaves its bench's berm.
    pub(crate) junction_flat: f64,
}

impl Default for RampSettings {
    fn default() -> Self {
        Self {
            width: 25.0,
            gradient: 10.0,
            start: None,
            climbs_clockwise: false,
            switchback: false,
            switchback_every: 3,
            turns_at: Vec::new(),
            inside_radius: 15.0,
            turn_widening: 0.0,
            from: None,
            to: None,
            bands: Vec::new(),
            exit_flat: 0.0,
            junction_flat: 0.0,
        }
    }
}

impl RampSettings {
    /// The design engine's ramp, starting at `default_start` unless its
    /// start has been picked.
    pub(crate) fn spec(&self, default_start: DVec2) -> RampSpec {
        RampSpec {
            width: self.width,
            gradient: self.gradient / 100.0,
            from: self.from,
            to: self.to,
            bands: self.bands.clone(),
            exit_flat: self.exit_flat,
            junction_flat: self.junction_flat,
            turns_at: if self.switchback { self.turns_at.clone() } else { Vec::new() },
            start: self.start.unwrap_or(default_start),
            climbs_clockwise: self.climbs_clockwise,
            layout: if self.switchback {
                Layout::Switchback {
                    every: self.switchback_every,
                    inside_radius: self.inside_radius,
                    widening: self.turn_widening,
                }
            } else {
                Layout::Spiral
            },
        }
    }
}

impl Default for PitDesignSettings {
    fn default() -> Self {
        Self {
            kind: Kind::Pit,
            direction: Direction::Up,
            bench_height: 15.0,
            face_angle: 70.0,
            berm_width: 8.0,
            bench_rules: BenchRules::default(),
            wall_radius: 15.0,
            ramps: vec![RampSettings::default()],
            stop: StopMode::Benches,
            benches: 8,
            stop_elevation: 0.0,
            topography: None,
        }
    }
}

impl PitDesignSettings {
    /// The design engine's settings, a ramp starting at `default_start`
    /// unless its start has been picked.
    pub(crate) fn spec(&self, default_start: DVec2) -> Spec {
        Spec {
            kind: self.kind,
            direction: self.direction,
            bench_height: self.bench_height,
            face_angle: self.face_angle,
            berm_width: self.berm_width,
            rules: self.bench_rules.clone(),
            wall_radius: self.wall_radius,
            ramps: self.ramps.iter().map(|ramp| ramp.spec(default_start)).collect(),
            stop: match self.stop {
                StopMode::Benches => Stop::Benches(self.benches),
                StopMode::Elevation => Stop::Elevation(self.stop_elevation),
                StopMode::Daylight => Stop::Daylight,
            },
        }
    }

    /// Whether the design can be built until it meets the ground: a pit
    /// built up from its floor, or a dump built down from its top.
    pub(crate) fn can_daylight(&self) -> bool {
        matches!((self.kind, self.direction), (Kind::Pit, Direction::Up) | (Kind::Dump, Direction::Down))
    }

    /// Whether the design is built until it meets the ground.
    pub(crate) fn daylights(&self) -> bool {
        self.stop == StopMode::Daylight && self.can_daylight()
    }

    /// The topography the design depends on, if it depends on one: the
    /// ground it daylights against, which its surface is also trimmed to.
    pub(crate) fn ground(&self) -> Option<TriangulationId> {
        self.topography.filter(|_| self.daylights())
    }
}
