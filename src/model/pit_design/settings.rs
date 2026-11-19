//! What a pit design's scenario sets: everything the engine needs beyond
//! the string it starts from, in the units people give it.

use glam::DVec2;
use serde::{Deserialize, Serialize};

use super::{Direction, Layout, RampSpec, Spec, Stop};
use crate::model::triangulation::TriangulationId;

/// Where a design stops, as a scenario sets it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum StopMode {
    Benches,
    Elevation,
    Daylight,
}

/// Everything a scenario sets. Two designs from the same string and equal
/// settings are the same design.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct PitDesignSettings {
    pub(crate) direction: Direction,
    pub(crate) bench_height: f64,
    /// Degrees from horizontal.
    pub(crate) face_angle: f64,
    pub(crate) berm_width: f64,
    pub(crate) wall_radius: f64,
    pub(crate) ramp: bool,
    pub(crate) ramp_width: f64,
    /// Rise over run, as a percentage.
    pub(crate) ramp_gradient: f64,
    /// Where the ramp leaves the string, in plan; the string's first point
    /// if none has been picked.
    pub(crate) ramp_start: Option<DVec2>,
    pub(crate) climbs_clockwise: bool,
    pub(crate) switchback: bool,
    /// Benches climbed between turns.
    pub(crate) switchback_every: u32,
    pub(crate) inside_radius: f64,
    pub(crate) stop: StopMode,
    pub(crate) benches: u32,
    pub(crate) stop_elevation: f64,
    /// The ground a design built up daylights against.
    pub(crate) topography: Option<TriangulationId>,
    /// Whether building the design also builds its surface.
    pub(crate) surface: bool,
    pub(crate) trim_to_topography: bool,
}

impl Default for PitDesignSettings {
    fn default() -> Self {
        Self {
            direction: Direction::Up,
            bench_height: 15.0,
            face_angle: 70.0,
            berm_width: 8.0,
            wall_radius: 15.0,
            ramp: true,
            ramp_width: 25.0,
            ramp_gradient: 10.0,
            ramp_start: None,
            climbs_clockwise: false,
            switchback: false,
            switchback_every: 3,
            inside_radius: 15.0,
            stop: StopMode::Benches,
            benches: 8,
            stop_elevation: 0.0,
            topography: None,
            surface: true,
            trim_to_topography: true,
        }
    }
}

impl PitDesignSettings {
    /// The design engine's settings, the ramp starting at `default_start`
    /// unless one has been picked.
    pub(crate) fn spec(&self, default_start: DVec2) -> Spec {
        Spec {
            direction: self.direction,
            bench_height: self.bench_height,
            face_angle: self.face_angle,
            berm_width: self.berm_width,
            wall_radius: self.wall_radius,
            ramp: self.ramp.then(|| RampSpec {
                width: self.ramp_width,
                gradient: self.ramp_gradient / 100.0,
                start: self.ramp_start.unwrap_or(default_start),
                climbs_clockwise: self.climbs_clockwise,
                layout: if self.switchback {
                    Layout::Switchback {
                        every: self.switchback_every,
                        inside_radius: self.inside_radius,
                    }
                } else {
                    Layout::Spiral
                },
            }),
            stop: match self.stop {
                StopMode::Benches => Stop::Benches(self.benches),
                StopMode::Elevation => Stop::Elevation(self.stop_elevation),
                StopMode::Daylight => Stop::Daylight,
            },
        }
    }

    /// Whether the design is built against the ground: built up, until it
    /// daylights.
    pub(crate) fn daylights(&self) -> bool {
        self.stop == StopMode::Daylight && self.direction == Direction::Up
    }

    /// The topography the design depends on, if it depends on one: the
    /// ground it daylights against, which its surface is also trimmed to.
    pub(crate) fn ground(&self) -> Option<TriangulationId> {
        self.topography.filter(|_| self.daylights())
    }

    /// Whether the surface is trimmed to the topography: only a design that
    /// daylights against one is.
    pub(crate) fn trims(&self) -> bool {
        self.surface && self.trim_to_topography && self.ground().is_some()
    }
}
