//! What a bench's walls are cut to, place by place round the pit: the face
//! angle and berm width a wall takes from the way it faces (slope sectors)
//! and from where it stands (slope regions), over what its bench takes from
//! its zone and the design.
//!
//! Each value comes from the first of these that sets it: a slope region
//! the wall stands in, the zone the bench starts in, the sector the wall
//! faces into, the design. A region's values blend into those round it over
//! its blend distance, half inside its edge and half out; sectors blend
//! into their neighbours round the compass. A catch berm, and the first
//! bench's want of one, hold whatever the wall.

use glam::DVec2;
use serde::{Deserialize, Serialize};

use super::{Spec, ring, ring::EdgeOffset};

/// The face angle and berm width walls facing one bearing take.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Sector {
    /// The bearing the wall faces, as its dip direction, in degrees from
    /// north: the pit's north wall faces south, 180.
    pub(crate) bearing: f64,
    /// Degrees from horizontal.
    pub(crate) face_angle: f64,
    pub(crate) berm_width: f64,
    /// The inter-ramp angle the walls facing this way should come to, in
    /// degrees, to check the design against: the design's own target if
    /// none.
    #[serde(default)]
    pub(crate) inter_ramp: Option<f64>,
}

/// What the walls facing one way come to overall, from the string the
/// design starts from to its last ring, measured square out from the
/// string every so far round it, between ramps.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct WallReport {
    /// The bearing the walls face: a slope sector's, or the middle of a
    /// compass octant's.
    pub(crate) bearing: f64,
    /// How many places it was measured at.
    pub(crate) samples: u32,
    /// The inter-ramp angle the benches were cut to make there, on average,
    /// in degrees.
    pub(crate) designed: f64,
    /// The inter-ramp angle they make, on average.
    pub(crate) achieved: f64,
    /// The steepest they make anywhere: a short stack between two passes of
    /// the ramp, with fewer berms in it, is steeper than a tall one.
    pub(crate) steepest: f64,
    /// How much steeper, on average, the stacks come out than their benches
    /// were cut to make, in degrees; negative where they come out flatter.
    /// A stack just above a pass of the ramp, its lowest bench partly taken
    /// by the road, is a little steeper than cut, so one stack alone says
    /// little; the walls as a whole steeper than cut say something is wrong.
    pub(crate) over_cut: f64,
    /// The angle asked for, if one was.
    pub(crate) target: Option<f64>,
    /// Where they come out steepest: half way up and across that stack.
    #[serde(default)]
    pub(crate) steepest_at: Option<glam::DVec3>,
}

impl WallReport {
    /// Whether the walls come out steeper than they should, on average:
    /// than the target asked for, or than their benches were cut to make.
    pub(crate) fn too_steep(&self) -> bool {
        self.target.is_some_and(|target| self.achieved > target + STEEPER_BY) || self.over_cut > STEEPER_BY
    }
}

/// How much steeper than it should be a wall may come out before it is
/// called too steep, in degrees.
pub(crate) const STEEPER_BY: f64 = 0.5;

/// An area in plan whose walls are cut otherwise, such as a fault zone or
/// weaker rock on one side of the pit.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct Region {
    /// Its outline in plan, closed.
    pub(crate) outline: Vec<DVec2>,
    /// Degrees from horizontal.
    pub(crate) face_angle: Option<f64>,
    pub(crate) berm_width: Option<f64>,
}

impl Region {
    /// How far `point` stands inside the outline: negative outside.
    fn depth(&self, point: DVec2) -> f64 {
        let count = self.outline.len();
        let mut nearest = f64::INFINITY;
        let mut inside = false;
        for index in 0..count {
            let (a, b) = (self.outline[index], self.outline[(index + 1) % count]);
            if (a.y > point.y) != (b.y > point.y) && point.x < a.x + (point.y - a.y) / (b.y - a.y) * (b.x - a.x) {
                inside = !inside;
            }
            let (foot, _) = crate::model::kernel::project_onto_segment(point, a, b);
            nearest = nearest.min(foot.distance(point));
        }
        if inside { nearest } else { -nearest }
    }

    /// How much of the region's values hold at `point`, blending over
    /// `blend` across its edge: all of them deep inside, none well outside.
    fn weight(&self, point: DVec2, blend: f64) -> f64 {
        if self.outline.len() < 3 {
            return 0.0;
        }
        let depth = self.depth(point);
        if blend <= 0.0 {
            return if depth > 0.0 { 1.0 } else { 0.0 };
        }
        (0.5 + depth / blend).clamp(0.0, 1.0)
    }

    /// The outline's extent in plan, widened by `margin`.
    fn reach(&self, margin: f64) -> (DVec2, DVec2) {
        let low = self.outline.iter().fold(DVec2::splat(f64::INFINITY), |low, point| low.min(*point));
        let high = self.outline.iter().fold(DVec2::splat(f64::NEG_INFINITY), |high, point| high.max(*point));
        (low - DVec2::splat(margin), high + DVec2::splat(margin))
    }
}

/// The value of each sector's `value` for a wall facing `bearing` degrees:
/// between two sectors' bearings, the share of the way from one to the
/// other round the compass.
fn round_compass(sectors: &[Sector], bearing: f64, value: impl Fn(&Sector) -> f64) -> Option<f64> {
    let first = sectors.first()?;
    if sectors.len() == 1 {
        return Some(value(first));
    }
    let bearing = bearing.rem_euclid(360.0);
    // The sector at or before the bearing, going clockwise, and the one after.
    let before = sectors
        .iter()
        .min_by(|a, b| (bearing - a.bearing).rem_euclid(360.0).total_cmp(&(bearing - b.bearing).rem_euclid(360.0)))?;
    let after = sectors
        .iter()
        .min_by(|a, b| (a.bearing - bearing).rem_euclid(360.0).total_cmp(&(b.bearing - bearing).rem_euclid(360.0)))?;
    let span = (after.bearing - before.bearing).rem_euclid(360.0);
    if span <= 1.0e-9 {
        return Some(value(before));
    }
    let share = (bearing - before.bearing).rem_euclid(360.0) / span;
    Some(value(before) + (value(after) - value(before)) * share)
}

/// Which of `bins` (bearings in degrees) a wall facing `bearing` is
/// counted with: the nearest round the compass.
pub(crate) fn bin_of(bins: &[f64], bearing: f64) -> usize {
    let off = |bin: f64| {
        let turn = (bearing - bin).rem_euclid(360.0);
        turn.min(360.0 - turn)
    };
    (0..bins.len()).min_by(|a, b| off(bins[*a]).total_cmp(&off(bins[*b]))).unwrap_or(0)
}

/// The bearing, in degrees from north, of a direction in plan.
pub(crate) fn bearing_of(direction: DVec2) -> f64 {
    direction.x.atan2(direction.y).to_degrees().rem_euclid(360.0)
}

/// What one bench's walls are cut to: its height, what its zone sets, and
/// the berm it leaves whatever the wall.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Walls<'a> {
    pub(crate) spec: &'a Spec,
    pub(crate) height: f64,
    /// The zone's face angle, if it sets one.
    pub(crate) zone_face: Option<f64>,
    /// The zone's berm width, if it sets one.
    pub(crate) zone_berm: Option<f64>,
    /// The berm every wall leaves, whatever its own: none before the first
    /// bench, a catch berm's width on a catch bench.
    pub(crate) fixed_berm: Option<f64>,
    /// The tightest the walls may turn: the design's, or the zone's.
    pub(crate) wall_radius: f64,
    /// Whether a bench's start ring has its noses rounded for its face, so
    /// the ring the face reaches comes out round them at `wall_radius`.
    pub(crate) round_noses: bool,
}

impl Walls<'_> {
    /// Whether every wall of the bench is cut alike.
    pub(crate) fn alike(&self) -> bool {
        self.spec.rules.sectors.is_empty() && self.spec.rules.regions.is_empty()
    }

    /// The face angle and berm width of a wall at `point` facing `facing`
    /// (a direction in plan, the way the face looks).
    pub(crate) fn at(&self, point: DVec2, facing: DVec2) -> (f64, f64) {
        let spec = self.spec;
        let rules = &spec.rules;
        let bearing = || bearing_of(facing);
        let mut face = self
            .zone_face
            .or_else(|| round_compass(&rules.sectors, bearing(), |sector| sector.face_angle))
            .unwrap_or(spec.face_angle);
        let mut berm = self
            .zone_berm
            .or_else(|| round_compass(&rules.sectors, bearing(), |sector| sector.berm_width))
            .unwrap_or(spec.berm_width);
        for region in &rules.regions {
            let weight = region.weight(point, rules.blend);
            if weight <= 0.0 {
                continue;
            }
            if let Some(angle) = region.face_angle {
                face += (angle - face) * weight;
            }
            if let Some(width) = region.berm_width {
                berm += (width - berm) * weight;
            }
        }
        (face, self.fixed_berm.unwrap_or(berm))
    }

    /// The horizontal run of the face at `point` facing `facing`.
    pub(crate) fn face_at(&self, point: DVec2, facing: DVec2) -> f64 {
        super::face_run(self.height, self.at(point, facing).0)
    }

    /// The berm's width at `point` facing `facing`.
    pub(crate) fn berm_at(&self, point: DVec2, facing: DVec2) -> f64 {
        self.at(point, facing).1
    }

    /// These walls along one ring, ready to be asked about any point of it.
    pub(crate) fn on<'b>(&'b self, ring: &'b [DVec2]) -> OnRing<'b> {
        OnRing {
            walls: self,
            ring,
            facings: (!self.spec.rules.sectors.is_empty()).then(|| Facings::new(ring)),
        }
    }

    /// A ring set off by `distances`: as [`ring::offset`] where the walls are
    /// alike, and where they vary, as [`ring::offset_varying`], or failing
    /// that (a sharp corner left by an old head wall, say), as
    /// [`ring::offset`] too, whose folds [`Self::unfolded`] takes out.
    pub(crate) fn offset(&self, ring: &[DVec2], distances: &[EdgeOffset]) -> Vec<Vec<DVec2>> {
        if self.alike() {
            return ring::offset(ring, distances);
        }
        let met = ring::offset_varying(ring, distances);
        if met.is_empty() { ring::offset(ring, distances) } else { met }
    }

    /// A ring set off by these walls with its folds taken out, where they
    /// vary: see [`ring::without_folds`]. As it is where they are alike.
    pub(crate) fn unfolded(&self, ring: Vec<DVec2>) -> Vec<DVec2> {
        if self.alike() { ring } else { ring::without_folds(ring, FOLD) }
    }

    /// Whether every wall leaves no berm.
    pub(crate) fn no_berm(&self) -> bool {
        match self.fixed_berm {
            Some(width) => width == 0.0,
            None => self.alike() && self.zone_berm.unwrap_or(self.spec.berm_width) == 0.0,
        }
    }

    /// Where to split a ring's edges so the walls are followed along them,
    /// as parameters of the ring: every [`FACING_STEP`] where the way a wall
    /// faces is turning, and every so far along an edge that comes near a
    /// slope region, so its blend is followed. None where every wall is
    /// alike.
    pub(crate) fn splits(&self, ring: &[DVec2]) -> Vec<f64> {
        let rules = &self.spec.rules;
        let count = ring.len();
        let mut splits = Vec::new();
        if !rules.sectors.is_empty() {
            // Within a window's length of either end of an edge, the way the
            // wall faces turns towards the next; every so far there, so the
            // offset follows it.
            for index in 0..count {
                let length = ring[index].distance(ring[(index + 1) % count]);
                let mut add = |along: f64| {
                    if along > ring::SAME_POINT && along < length - ring::SAME_POINT {
                        splits.push(index as f64 + along / length);
                    }
                };
                if length <= 2.0 * FACING_WINDOW {
                    let pieces = (length / FACING_STEP).ceil() as usize;
                    for piece in 1..pieces {
                        add(length * piece as f64 / pieces as f64);
                    }
                } else {
                    let pieces = (FACING_WINDOW / FACING_STEP).ceil() as usize;
                    for piece in 1..=pieces {
                        let along = FACING_WINDOW * piece as f64 / pieces as f64;
                        add(along);
                        add(length - along);
                    }
                }
            }
        }
        if rules.regions.is_empty() {
            return splits;
        }
        let step = (rules.blend / 4.0).clamp(1.0, 10.0);
        let reaches: Vec<(DVec2, DVec2)> = rules.regions.iter().map(|region| region.reach(rules.blend * 0.5 + step)).collect();
        for index in 0..count {
            let (a, b) = (ring[index], ring[(index + 1) % count]);
            let (low, high) = (a.min(b), a.max(b));
            if !reaches.iter().any(|(reach_low, reach_high)| low.cmple(*reach_high).all() && high.cmpge(*reach_low).all()) {
                continue;
            }
            let pieces = (a.distance(b) / step).ceil() as usize;
            splits.extend((1..pieces).map(|piece| index as f64 + piece as f64 / pieces as f64));
        }
        splits
    }

    /// Each edge's offset by `distance` of the wall at its ends, outward
    /// where `sign` is positive. A point takes the way its wall faces over
    /// [`FACING_WINDOW`] either side of it, so the edges meeting there are set
    /// off alike and the offset has no step in it: a wall's values change
    /// round a corner, not at it, and a kink in a wall changes nothing. Split
    /// as [`Self::splits`] says first, the offset follows them closely.
    pub(crate) fn offsets(&self, ring: &[DVec2], sign: f64, distance: impl Fn(&Self, DVec2, DVec2) -> f64) -> Vec<EdgeOffset> {
        let count = ring.len();
        let facings = self.facings(ring);
        let at: Vec<f64> = (0..count).map(|index| distance(self, ring[index], facings[index])).collect();
        (0..count)
            .map(|index| EdgeOffset {
                start: sign * at[index],
                end: sign * at[(index + 1) % count],
            })
            .collect()
    }

    /// The way the wall faces at each point of a ring, as the offset takes
    /// it. Only sectors care: anything will do without them.
    pub(crate) fn facings(&self, ring: &[DVec2]) -> Vec<DVec2> {
        let count = ring.len();
        if self.spec.rules.sectors.is_empty() {
            return (0..count).map(|index| facing(ring, index)).collect();
        }
        let facings = Facings::new(ring);
        (0..count).map(|index| facings.at(facings.arcs[index])).collect()
    }

    /// The least and greatest face angle and berm width round a ring, at
    /// its points and half way along each edge.
    pub(crate) fn range(&self, ring: &[DVec2]) -> ([f64; 2], [f64; 2]) {
        let mut face = [f64::INFINITY, f64::NEG_INFINITY];
        let mut berm = [f64::INFINITY, f64::NEG_INFINITY];
        let on = self.on(ring);
        for index in 0..ring.len() {
            for parameter in [index as f64, index as f64 + 0.5] {
                let (point, facing) = on.place(parameter);
                let (angle, width) = self.at(point, facing);
                face = [face[0].min(angle), face[1].max(angle)];
                berm = [berm[0].min(width), berm[1].max(width)];
            }
        }
        (face, berm)
    }
}

/// How far either side of a point of a wall the way it faces is taken
/// over: a wall's own orientation, not that of a kink in it.
pub(crate) const FACING_WINDOW: f64 = 20.0;

/// The longest edge beside a fold taken out of a ring set off by walls
/// that vary.
const FOLD: f64 = 0.5;

/// A ring turning more than this at both ends of an edge, one way then
/// back, steps across there rather than running on as a wall.
const JOG: f64 = std::f64::consts::FRAC_PI_4;

/// How far apart a ring is split where the way its walls face turns.
const FACING_STEP: f64 = 5.0;

/// A bench's walls along one ring: the face's run and the berm's width at
/// any point of it, the wall facing as [`Walls::facings`] has it.
pub(crate) struct OnRing<'a> {
    walls: &'a Walls<'a>,
    ring: &'a [DVec2],
    /// How the ring faces, where that counts: with sectors.
    facings: Option<Facings<'a>>,
}

impl OnRing<'_> {
    /// The face's run at the ring's point `parameter`.
    pub(crate) fn face(&self, parameter: f64) -> f64 {
        let (point, facing) = self.place(parameter);
        self.walls.face_at(point, facing)
    }

    /// The berm's width at the ring's point `parameter`.
    pub(crate) fn berm(&self, parameter: f64) -> f64 {
        let (point, facing) = self.place(parameter);
        self.walls.berm_at(point, facing)
    }

    /// The way the wall faces at the ring's point `parameter`.
    pub(crate) fn facing(&self, parameter: f64) -> DVec2 {
        self.place(parameter).1
    }

    /// The point at `parameter`, and the way its wall faces.
    fn place(&self, parameter: f64) -> (DVec2, DVec2) {
        let count = self.ring.len();
        let parameter = parameter.rem_euclid(count as f64);
        let point = ring::point_at(self.ring, parameter);
        match &self.facings {
            Some(facings) => (point, facings.at(ring::arc_at(&facings.arcs, parameter))),
            None => (point, facing(self.ring, (parameter.floor() as usize).min(count - 1))),
        }
    }
}

/// The way a ring's walls face, summed along it: what finds how they face
/// on average over any stretch of it at the cost of two lookups.
struct Facings<'a> {
    ring: &'a [DVec2],
    arcs: Vec<f64>,
    /// The facing of each edge times its length, summed to each point.
    sums: Vec<DVec2>,
}

impl<'a> Facings<'a> {
    fn new(ring: &'a [DVec2]) -> Self {
        let arcs = ring::arcs(ring);
        let count = ring.len();
        // How the ring turns at each point, left positive.
        let turns: Vec<f64> = (0..count)
            .map(|index| {
                let (before, after) = (ring::direction(ring, (index + count - 1) % count), ring::direction(ring, index));
                before.perp_dot(after).atan2(before.dot(after))
            })
            .collect();
        let mut sums = Vec::with_capacity(count + 1);
        let mut sum = DVec2::ZERO;
        sums.push(sum);
        for index in 0..count {
            // A jog (sharply one way at one end and back at the other) is
            // a step across a road, not a wall: it counts for nothing.
            let (into, out_of) = (turns[index], turns[(index + 1) % count]);
            let jog = into.abs() > JOG && out_of.abs() > JOG && into.signum() != out_of.signum();
            if !jog {
                sum += facing(ring, index) * (arcs[index + 1] - arcs[index]);
            }
            sums.push(sum);
        }
        Self { ring, arcs, sums }
    }

    /// The sum from the ring's first point to `arc` round it, `arc` within
    /// one perimeter either side of it.
    fn sum_to(&self, arc: f64) -> DVec2 {
        let count = self.ring.len();
        let around = self.arcs[count];
        let laps = (arc / around).floor();
        let arc = arc - laps * around;
        let index = self.arcs.partition_point(|start| *start <= arc).saturating_sub(1).min(count - 1);
        self.sums[count] * laps + self.sums[index] + facing(self.ring, index) * (arc - self.arcs[index])
    }

    /// The way the walls face, on average, over [`FACING_WINDOW`] either
    /// side of the point `at` round the ring.
    fn at(&self, at: f64) -> DVec2 {
        let window = FACING_WINDOW.min(self.arcs[self.ring.len()] * 0.5);
        let sum = self.sum_to(at + window) - self.sum_to(at - window);
        let index = (ring::parameter_at(&self.arcs, at.rem_euclid(self.arcs[self.ring.len()])).floor() as usize).min(self.ring.len() - 1);
        sum.try_normalize().unwrap_or_else(|| facing(self.ring, index))
    }
}

/// The way the face on edge `index` of an anticlockwise ring looks: into
/// the pit, away from the ring's outside.
pub(crate) fn facing(ring: &[DVec2], index: usize) -> DVec2 {
    -ring::outward(ring, index)
}
