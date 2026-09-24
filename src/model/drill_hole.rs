use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    hash::{Hash, Hasher},
    path::PathBuf,
    sync::Arc,
};

use glam::{DQuat, DVec2, DVec3};
use serde::{Deserialize, Serialize};

use crate::{
    i18n::{tr, tr_format},
    model::{formats::csv_drill_hole::CsvDrillFileMapping, project::ProjectItemState},
};

/// How much wider than the hole itself the collar marker is drawn.
pub(crate) const COLLAR_MARKER_RADIUS_SCALE: f64 = 5.0;
/// Smallest on-screen diameter used to draw a drill-hole trace.
pub(crate) const MIN_RENDER_PIXEL_DIAMETER: f32 = 2.0;
/// Independent screen-space floor: collars shrink to dots in overview views
/// instead of magnifying the trace's minimum diameter by the marker scale.
pub(crate) const COLLAR_MARKER_MIN_PIXEL_DIAMETER: f32 = 3.0;
/// World-space collar radius for datasets that do not provide a physical hole
/// diameter. Roughly matches a 240 mm production hole after the marker scale.
pub(crate) const COLLAR_MARKER_FALLBACK_RADIUS: f64 = 0.6;
pub(crate) const COLLAR_MARKER_OUTLINE_COLOR: [f32; 3] = [0.086, 0.376, 0.851];
pub(crate) const COLLAR_MARKER_FILL_COLOR: [f32; 3] = [1.0, 1.0, 1.0];
/// How many stops a numeric ramp may carry. A limit on the ramp only: a
/// categorical field colours every code it has.
pub(crate) const MAX_DRILL_COLOR_STOPS: usize = 12;
/// Distinct codes above which a categorical column is named on load as
/// likely free text. Nothing is dropped at any count.
pub(crate) const WIDE_CATEGORY_FIELD_HINT: usize = 256;
/// Upper bound for an interactively generated blast pattern. It keeps a bad
/// unit/spacing entry from building millions of preview primitives on the UI
/// thread while remaining comfortably above ordinary production rounds.
pub(crate) const MAX_PATTERN_HOLES: usize = 25_000;
/// How thick a tie-in connector is drawn, as a multiple of the radius of the
/// holes it joins. Its physical width follows the pattern, while the renderer
/// applies the same screen-space floor as a drill-hole trace.
pub(crate) const TIE_RADIUS_SCALE: f64 = 1.5;
/// A disc's diameter, in metres, where a set has not chosen one: ten times
/// an HQ core hole, so a disc never reads as the hole itself, and small
/// beside the closest drill spacings, so neighbouring discs never touch.
/// Far out the pixel floor below sets the size, not this.
pub(crate) const DEFAULT_DISC_DIAMETER: f64 = 1.0;
/// What the disc diameter control accepts, in metres.
pub(crate) const DISC_DIAMETER_RANGE: std::ops::RangeInclusive<f64> = 0.1..=10.0;
/// The string's on-screen width where a set has not chosen one.
pub(crate) const DEFAULT_STRING_PIXEL_WIDTH: f32 = 2.0;
/// What the string width control accepts, in pixels.
pub(crate) const STRING_PIXEL_WIDTH_RANGE: std::ops::RangeInclusive<f32> = 1.0..=8.0;
/// How much wider than the string a disc is always drawn, in pixels: two
/// pixels of colour either side of it however far the eye is.
pub(crate) const DISC_PIXEL_MARGIN: f32 = 4.0;
/// The least a disc is drawn along the string, in pixels, so a ply a few
/// centimetres thick still shows from kilometres away.
pub(crate) const DISC_MIN_PIXEL_LENGTH: f32 = 3.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) struct DrillHoleId(pub(crate) u64);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum DrillHoleSource {
    /// Deserialization-only compatibility for sessions created before DHD
    /// support was removed. These sources are never restored or opened.
    #[serde(rename = "Dhd")]
    LegacyDhd { path: PathBuf },
    Csv {
        name: String,
        files: Vec<CsvDrillFileMapping>,
        /// Filename-only browser identity. Native imports use their first
        /// mapped file path while the project retains the decoded dataset.
        #[serde(default)]
        browser_path: Option<PathBuf>,
    },
    /// A dataset decoded from an Open Mining Format container. It stays
    /// in-memory until exported or explicitly saved in another format.
    Omf { name: String, path: PathBuf },
}

impl DrillHoleSource {
    pub(crate) fn display_name(&self) -> String {
        match self {
            Self::LegacyDhd { path } => path
                .file_name()
                .and_then(|name| name.to_str())
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| tr!(literal = "Unsupported drillhole source")),
            Self::Csv { name, .. } => name.clone(),
            Self::Omf { name, .. } => name.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct TraceStation {
    pub(crate) depth: f64,
    pub(crate) position: DVec3,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) enum DrillValue {
    Numeric(f64),
    Category(String),
}

/// One interval as the site logged it, and its identity within its hole.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct LoggedInterval {
    pub(crate) from: f64,
    pub(crate) to: f64,
    pub(crate) values: BTreeMap<String, DrillValue>,
}

/// One interval as interpreted: what every reader, colour and section sees.
/// `logged` is what the site sent, or `None` when nothing has parted them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct DrillInterval {
    pub(crate) from: f64,
    pub(crate) to: f64,
    pub(crate) values: BTreeMap<String, DrillValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) logged: Option<LoggedInterval>,
}

impl DrillInterval {
    pub(crate) fn logged(&self) -> (f64, f64, &BTreeMap<String, DrillValue>) {
        match &self.logged {
            Some(logged) => (logged.from, logged.to, &logged.values),
            None => (self.from, self.to, &self.values),
        }
    }

    pub(crate) fn is_corrected(&self) -> bool {
        self.logged
            .as_ref()
            .is_some_and(|logged| logged.from != self.from || logged.to != self.to || logged.values != self.values)
    }
}

/// One surface connector: the delay laid between two holes, and which way the
/// round travels over it.
///
/// The product is carried by value rather than by [`crate::ui::state::DelayProductId`].
/// The palette is application configuration - its ids are handed out afresh
/// each run and its order is by delay - so an id stored here would repoint at
/// whatever product took its place. A tie is a record of what was actually
/// laid, which is why editing the palette leaves a tied round alone.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct TieIn {
    /// Index of the hole the signal arrives at first.
    pub(crate) from: usize,
    /// ...and of the one it fires onward into.
    pub(crate) to: usize,
    pub(crate) delay_ms: u32,
    pub(crate) product: String,
    pub(crate) color: [f32; 3],
}

impl TieIn {
    /// Whether this connector joins the same two holes as `other`, whichever
    /// way round either runs. Two holes are joined by one connector or none -
    /// there is nowhere to put a second - so this is the identity a new tie
    /// overwrites on.
    pub(crate) fn joins(&self, from: usize, to: usize) -> bool {
        (self.from == from && self.to == to) || (self.from == to && self.to == from)
    }
}

/// Where a round starts, and how long after the shot is fired it goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Initiation {
    pub(crate) hole: usize,
    pub(crate) delay_ms: u32,
}

/// Ties as a file holds them: keyed by hole name, because a hole's index is
/// only stable for as long as the dataset stays loaded - see [`DrillHoleRef`] -
/// and ties outlive that.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct StoredTieIns {
    #[serde(default)]
    pub(crate) ties: Vec<StoredTieIn>,
    /// Every hole that can start the round. New files use this collection;
    /// `initiation` below is retained only so projects written by the first
    /// tie-in implementation still open without losing their start point.
    #[serde(default)]
    pub(crate) initiations: Vec<StoredInitiation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) initiation: Option<StoredInitiation>,
}

impl StoredTieIns {
    pub(crate) fn is_empty(&self) -> bool {
        self.ties.is_empty() && self.initiations.is_empty() && self.initiation.is_none()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct StoredTieIn {
    pub(crate) from: String,
    pub(crate) to: String,
    pub(crate) delay_ms: u32,
    pub(crate) product: String,
    pub(crate) color: [f32; 3],
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct StoredInitiation {
    pub(crate) hole: String,
    pub(crate) delay_ms: u32,
}

/// Where a hole's orientation came from: measured, assumed, or unknown.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum OrientationSource {
    /// A survey record placed or steered the trace.
    Measured,
    /// No survey; the direction was set by hand or by design.
    Assumed,
    /// Nothing recorded; the trace is only a projection.
    #[default]
    Unknown,
}

impl OrientationSource {
    pub(crate) fn label(self) -> String {
        match self {
            Self::Measured => tr!(literal = "Measured"),
            Self::Assumed => tr!(literal = "Assumed"),
            Self::Unknown => tr!(literal = "Unknown"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct DrillHole {
    pub(crate) dhid: String,
    pub(crate) collar: DVec3,
    /// Source diameter. The 2 px visual floor is applied by the trace shader,
    /// without changing this physical value.
    pub(crate) diameter: Option<f64>,
    pub(crate) trace: Vec<TraceStation>,
    /// Explicit geometry coverage. Empty means the complete measured-depth
    /// trace is continuous.
    pub(crate) render_ranges: Vec<(f64, f64)>,
    pub(crate) intervals: Vec<DrillInterval>,
    /// Defaults to `Unknown` for a project saved before this field existed.
    #[serde(default)]
    pub(crate) orientation_source: OrientationSource,
}

/// Row arrangement used when filling a blast boundary with collars.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum DrillPatternLayout {
    #[default]
    Square,
    Staggered,
}

impl DrillPatternLayout {
    pub(crate) const ALL: [Self; 2] = [Self::Square, Self::Staggered];

    pub(crate) fn label(self) -> String {
        match self {
            Self::Square => tr!(literal = "Square"),
            Self::Staggered => tr!(literal = "Staggered"),
        }
    }
}

/// Fill an XY polygon with a drill grid rotated counter-clockwise from global
/// X. `spacing` runs within a row and `burden` separates rows; staggered rows
/// move half a spacing. Collars sit half a cell inside the rotated polygon
/// bounds and take their Z from the boundary's polygon plane.
pub(crate) fn generate_pattern_collars(
    boundary: &[DVec3],
    burden: f64,
    spacing: f64,
    rotation_degrees: f64,
    offset: DVec2,
    layout: DrillPatternLayout,
) -> Result<Vec<DVec3>, String> {
    if boundary.len() < 3 || boundary.iter().any(|point| !point.is_finite()) {
        return Err(tr!(literal = "Choose a valid closed polyline"));
    }
    if !burden.is_finite() || !spacing.is_finite() || burden <= 0.0 || spacing <= 0.0 {
        return Err(tr!(literal = "Burden and spacing must be greater than zero"));
    }
    if !rotation_degrees.is_finite() || !offset.is_finite() {
        return Err(tr!(literal = "Rotation and offsets must contain valid numbers"));
    }

    let centroid = boundary.iter().copied().sum::<DVec3>() / boundary.len() as f64;
    let (sin_rotation, cos_rotation) = rotation_degrees.to_radians().sin_cos();
    let grid_offset = DVec2::new(offset.x * cos_rotation + offset.y * sin_rotation, -offset.x * sin_rotation + offset.y * cos_rotation);
    let grid_boundary = boundary
        .iter()
        .map(|point| {
            let offset = point.truncate() - centroid.truncate();
            DVec3::new(
                offset.x * cos_rotation + offset.y * sin_rotation,
                -offset.x * sin_rotation + offset.y * cos_rotation,
                point.z,
            )
        })
        .collect::<Vec<_>>();
    let min_x = grid_boundary.iter().map(|point| point.x).fold(f64::INFINITY, f64::min);
    let max_x = grid_boundary.iter().map(|point| point.x).fold(f64::NEG_INFINITY, f64::max);
    let min_y = grid_boundary.iter().map(|point| point.y).fold(f64::INFINITY, f64::min);
    let max_y = grid_boundary.iter().map(|point| point.y).fold(f64::NEG_INFINITY, f64::max);
    let width = max_x - min_x;
    let height = max_y - min_y;
    let columns = ((width / spacing).ceil() as usize).max(1);
    let rows = ((height / burden).ceil() as usize).max(1);
    let cells = columns.saturating_mul(rows);
    if cells > MAX_PATTERN_HOLES.saturating_mul(20) {
        return Err(crate::i18n::tr_format!(
            literal = "This spacing would scan too many grid cells; increase burden or spacing (maximum %maximum% holes)",
            maximum = MAX_PATTERN_HOLES
        ));
    }

    // Newell's method gives a stable normal for either winding and polygons
    // with more than three vertices. A near-vertical/degenerate ring falls
    // back to the mean boundary elevation below.
    let mut normal = DVec3::ZERO;
    for index in 0..boundary.len() {
        let current = boundary[index];
        let next = boundary[(index + 1) % boundary.len()];
        normal.x += (current.y - next.y) * (current.z + next.z);
        normal.y += (current.z - next.z) * (current.x + next.x);
        normal.z += (current.x - next.x) * (current.y + next.y);
    }
    if normal.z.abs() <= (width * height).abs().max(1.0) * 1.0e-12 {
        return Err(tr!(literal = "The selected polyline has no usable XY area"));
    }
    let elevation = |x: f64, y: f64| {
        if normal.z.abs() > 1.0e-12 {
            centroid.z - (normal.x * (x - centroid.x) + normal.y * (y - centroid.y)) / normal.z
        } else {
            centroid.z
        }
    };

    let mut collars = Vec::with_capacity(cells.min(MAX_PATTERN_HOLES));
    let base_x = if width < spacing { (min_x + max_x) * 0.5 } else { min_x + spacing * 0.5 };
    let base_y = if height < burden { (min_y + max_y) * 0.5 } else { min_y + burden * 0.5 };
    // Reduce arbitrary offsets to one lattice period and start at the first
    // candidate inside the rotated bounds. This keeps large coordinate entry
    // values cheap while preserving their exact pattern phase.
    let first_x = base_x + grid_offset.x + ((min_x - base_x - grid_offset.x) / spacing).ceil() * spacing;
    let first_y = base_y + grid_offset.y + ((min_y - base_y - grid_offset.y) / burden).ceil() * burden;
    // Rows run in ascending Y, so a sweep keeps only the edges a row can
    // touch: the ones it crosses, plus any close enough to fall under the
    // on-boundary tolerance. A dense ring - a tessellated circle, say - then
    // costs one edge pass per row instead of one per candidate hole.
    struct BoundaryEdge {
        start: DVec2,
        end: DVec2,
        min_y: f64,
        max_y: f64,
    }
    const ON_EDGE_TOLERANCE: f64 = 1.0e-8;
    let mut edges = grid_boundary
        .iter()
        .enumerate()
        .map(|(index, point)| {
            let start = point.truncate();
            let end = grid_boundary[(index + 1) % grid_boundary.len()].truncate();
            BoundaryEdge {
                start,
                end,
                min_y: start.y.min(end.y),
                max_y: start.y.max(end.y),
            }
        })
        .collect::<Vec<_>>();
    edges.sort_unstable_by(|left, right| left.min_y.total_cmp(&right.min_y));

    let mut pending_edge = 0_usize;
    let mut active_edges: Vec<usize> = Vec::new();
    let mut crossings: Vec<f64> = Vec::new();
    let mut on_boundary: Vec<usize> = Vec::new();
    for row in 0..rows {
        let y = first_y + row as f64 * burden;
        if y >= max_y {
            break;
        }
        let stagger = if layout == DrillPatternLayout::Staggered && row % 2 == 1 { spacing * 0.5 } else { 0.0 };

        while pending_edge < edges.len() && edges[pending_edge].min_y <= y + ON_EDGE_TOLERANCE {
            active_edges.push(pending_edge);
            pending_edge += 1;
        }
        active_edges.retain(|&index| edges[index].max_y >= y - ON_EDGE_TOLERANCE);

        crossings.clear();
        on_boundary.clear();
        for &index in &active_edges {
            let BoundaryEdge { start, end, .. } = edges[index];
            if (start.y > y) != (end.y > y) {
                crossings.push((end.x - start.x) * (y - start.y) / (end.y - start.y) + start.x);
            }
            // The tolerance is a hair over an exact hit, so an edge can only
            // put a handful of columns on the boundary - walk those instead of
            // testing every column against every edge.
            let span = end - start;
            let length_squared = span.length_squared();
            if length_squared > 0.0 {
                let low = start.x.min(end.x) - ON_EDGE_TOLERANCE;
                let high = start.x.max(end.x) + ON_EDGE_TOLERANCE;
                let first_column = (((low - first_x - stagger) / spacing).ceil()).max(0.0) as usize;
                let last_column = (((high - first_x - stagger) / spacing).floor()).max(0.0) as usize;
                for column in first_column..=last_column.min(columns.saturating_sub(1)) {
                    let point = DVec2::new(first_x + column as f64 * spacing + stagger, y);
                    let along = ((point - start).dot(span) / length_squared).clamp(0.0, 1.0);
                    if point.distance_squared(start + span * along) <= ON_EDGE_TOLERANCE * ON_EDGE_TOLERANCE {
                        on_boundary.push(column);
                    }
                }
            }
        }
        crossings.sort_unstable_by(f64::total_cmp);
        on_boundary.sort_unstable();
        on_boundary.dedup();

        // Even-odd fill: a column is inside when an odd number of crossings
        // lie to its right, which one pointer walk resolves for the whole row.
        let mut crossing_index = 0_usize;
        let mut boundary_index = 0_usize;
        for column in 0..columns {
            let x = first_x + column as f64 * spacing + stagger;
            if x >= max_x {
                break;
            }
            while crossing_index < crossings.len() && crossings[crossing_index] <= x {
                crossing_index += 1;
            }
            while boundary_index < on_boundary.len() && on_boundary[boundary_index] < column {
                boundary_index += 1;
            }
            let on_edge = on_boundary.get(boundary_index).is_some_and(|&hit| hit == column);
            if on_edge || (crossings.len() - crossing_index) % 2 == 1 {
                let world_x = centroid.x + x * cos_rotation - y * sin_rotation;
                let world_y = centroid.y + x * sin_rotation + y * cos_rotation;
                collars.push(DVec3::new(world_x, world_y, elevation(world_x, world_y)));
                if collars.len() > MAX_PATTERN_HOLES {
                    return Err(crate::i18n::tr_format!(
                        literal = "Pattern exceeds the maximum of %maximum% holes; increase burden or spacing",
                        maximum = MAX_PATTERN_HOLES
                    ));
                }
            }
        }
    }
    if collars.is_empty() {
        return Err(tr!(literal = "No holes fit inside this boundary at the current burden and spacing"));
    }
    Ok(collars)
}

/// Everything a move rewrites in a hole: its collar and its trace. Nothing
/// else is touched - the intervals above all, whose per-interval value maps
/// are what makes a whole [`DrillHole`] expensive to copy - so a live preview
/// captures and rewrites only this.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct HolePlacement {
    pub(crate) collar: DVec3,
    pub(crate) trace: Vec<TraceStation>,
    pub(crate) orientation_source: OrientationSource,
}

/// Where a hole points, in the terms a drill plan is written in: `azimuth`
/// degrees clockwise from grid north, `dip` degrees from horizontal with down
/// negative. This is the convention [`project_tangent`] resolves a survey in,
/// so a hole read out of a file and a hole turned here describe themselves the
/// same way.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct HoleOrientation {
    pub(crate) azimuth: f64,
    pub(crate) dip: f64,
}

/// Limit for canonical dip readouts and typed drill-plan orientations.
/// Ring gestures retain unwrapped dip angles so they can pass through vertical.
pub(crate) const MAX_HOLE_DIP: f64 = 90.0;

/// How a Rotate Collar edit turns the holes it was handed.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) enum CollarRotation {
    /// Point every hole the same way, whatever each was pointing before - what
    /// the panel applies, a round being drilled at one angle.
    Absolute(HoleOrientation),
    /// Turn each hole from where it stood, so a pattern that was not uniform
    /// to begin with keeps its spread - what a ring drag produces.
    Delta { azimuth: f64, dip: f64 },
}

impl CollarRotation {
    /// The turn that leaves every hole exactly where it was, which is what
    /// reverting a Rotate Collar command writes.
    pub(crate) const IDENTITY: Self = Self::Delta { azimuth: 0.0, dip: 0.0 };

    pub(crate) fn is_identity(self) -> bool {
        matches!(self, Self::Delta { azimuth, dip } if azimuth == 0.0 && dip == 0.0)
    }
}

/// Which way a collar was set up, read off the first surveyed length below
/// it - the piece of the hole the rig actually aimed, rather than the
/// collar-to-toe chord, which on a hole that bends is neither.
///
/// A trace with no length below the collar has no direction to report.
fn trace_orientation(collar: DVec3, trace: &[TraceStation]) -> Option<HoleOrientation> {
    let direction = trace.iter().find_map(|station| {
        let offset = station.position - collar;
        (offset.length_squared() > 1.0e-12).then_some(offset)
    })?;
    Some(direction_orientation(direction))
}

/// Azimuth and dip of a direction, in the drill-plan convention.
///
/// A dead-vertical direction has no bearing to report and comes back as north,
/// the same stand-in [`resolve_trace`] falls back on for a survey that never
/// gave one.
pub(crate) fn direction_orientation(direction: DVec3) -> HoleOrientation {
    HoleOrientation {
        azimuth: direction.x.atan2(direction.y).to_degrees().rem_euclid(360.0),
        dip: direction.z.atan2(direction.x.hypot(direction.y)).to_degrees(),
    }
}

/// The world rotation taking a hole pointing `from` to one pointing `to`: a
/// swing about the vertical, then a tilt within the vertical plane the swing
/// left it in.
///
/// Decomposed that way rather than as a shortest-arc rotation because the two
/// halves are the two numbers on the drill plan: each gizmo ring drives one of
/// them, and neither disturbs the other.
fn orientation_rotation(from: HoleOrientation, to: HoleOrientation) -> DQuat {
    // Azimuth runs clockwise seen from above, which is the negative direction
    // about +Z.
    let swing = DQuat::from_rotation_z(-(to.azimuth - from.azimuth).to_radians());
    // Horizontal axis square to the destination bearing: turning about it is
    // what raises or drops the toe.
    let bearing = to.azimuth.to_radians();
    let tilt_axis = DVec3::new(bearing.cos(), -bearing.sin(), 0.0);
    DQuat::from_axis_angle(tilt_axis, (to.dip - from.dip).to_radians()) * swing
}

impl HolePlacement {
    /// Where the collar stands. Mirrors [`DrillHole::collar_position`]: the
    /// trace's first station is it, and `collar` only stands in for a hole
    /// that arrived without a trace.
    pub(crate) fn collar_position(&self) -> DVec3 {
        self.trace.first().map_or(self.collar, |station| station.position)
    }

    /// Which way the collar was set up. See [`trace_orientation`].
    pub(crate) fn orientation(&self) -> Option<HoleOrientation> {
        trace_orientation(self.collar_position(), &self.trace)
    }

    /// What `rotation` would leave this hole pointing at. `None` where the
    /// hole has no direction of its own to turn from.
    pub(crate) fn rotated_orientation(&self, rotation: CollarRotation) -> Option<HoleOrientation> {
        let from = self.orientation()?;
        Some(match rotation {
            CollarRotation::Absolute(target) => target,
            CollarRotation::Delta { azimuth, dip } => HoleOrientation {
                azimuth: (from.azimuth + azimuth).rem_euclid(360.0),
                dip: from.dip + dip,
            },
        })
    }
}

impl DrillHole {
    /// Capture where the hole stands, for a preview to rewrite it from.
    pub(crate) fn placement(&self) -> HolePlacement {
        HolePlacement {
            collar: self.collar,
            trace: self.trace.clone(),
            orientation_source: self.orientation_source,
        }
    }

    /// Put the hole back at `placement` translated by `delta` - a zero delta
    /// therefore restores it exactly. The trace is copied into the existing
    /// allocation rather than replacing it. Interval geometry is measured down
    /// the trace rather than in world space, so it needs no adjustment.
    pub(crate) fn set_placement(&mut self, placement: &HolePlacement, delta: DVec3) {
        self.collar = placement.collar + delta;
        self.trace.clone_from(&placement.trace);
        self.orientation_source = placement.orientation_source;
        if delta != DVec3::ZERO {
            for station in &mut self.trace {
                station.position += delta;
            }
        }
    }

    /// Put the hole back at `placement` turned by `rotation` about its own
    /// collar - [`CollarRotation::IDENTITY`] therefore restores it exactly.
    ///
    /// The collar never moves: the whole trace swings rigidly about it, survey
    /// curvature and all, so a turned hole is the same hole aimed elsewhere
    /// rather than a straightened one. Interval geometry is measured down the
    /// trace rather than in world space, so it needs no adjustment - the same
    /// reason [`Self::set_placement`] leaves it alone.
    pub(crate) fn set_rotated_placement(&mut self, placement: &HolePlacement, rotation: CollarRotation) {
        self.collar = placement.collar;
        self.trace.clone_from(&placement.trace);
        self.orientation_source = placement.orientation_source;
        if rotation.is_identity() {
            // The restore path, taken on every rollback: the trace copy above
            // is already the whole of it.
            return;
        }
        let Some(from) = placement.orientation() else {
            // Nothing below the collar to aim, so there is nothing to turn.
            return;
        };
        let Some(to) = placement.rotated_orientation(rotation) else {
            return;
        };
        if to == from {
            return;
        }
        let quat = orientation_rotation(from, to);
        let pivot = placement.collar_position();
        for station in &mut self.trace {
            station.position = pivot + quat * (station.position - pivot);
        }
        self.orientation_source = OrientationSource::Assumed;
    }

    /// The physical world radius of the hole. A dataset without diameters uses
    /// a nominal radius for collars and ties; traces receive their independent
    /// screen-space floor in the renderer.
    pub(crate) fn render_radius(&self) -> f64 {
        self.diameter.map_or(COLLAR_MARKER_FALLBACK_RADIUS / COLLAR_MARKER_RADIUS_SCALE, |diameter| diameter * 0.5)
    }

    /// Where the collar stands. The trace's first station is it; `collar`
    /// only stands in for a dataset that arrived without a trace.
    pub(crate) fn collar_position(&self) -> DVec3 {
        self.trace.first().map_or(self.collar, |station| station.position)
    }

    /// Which way the hole was set up. See [`trace_orientation`]. Asked of the
    /// hole rather than of a captured placement so a per-frame readout costs
    /// no trace copy.
    pub(crate) fn orientation(&self) -> Option<HoleOrientation> {
        trace_orientation(self.collar_position(), &self.trace)
    }

    /// Where the trace stands at `depth`. Binary search, not a walk from the
    /// collar, so building a hole stops being quadratic in its station count.
    pub(crate) fn position_at_depth(&self, depth: f64) -> Option<DVec3> {
        let first = *self.trace.first()?;
        if depth <= first.depth {
            return Some(first.position);
        }
        let index = self.trace.partition_point(|station| station.depth < depth);
        // Past the toe there is no bracketing pair, and a depth that is not a
        // number lands at zero: both fall through to the last station.
        let Some(&[a, b]) = index.checked_sub(1).and_then(|above| self.trace.get(above..=index)) else {
            return self.trace.last().map(|station| station.position);
        };
        let span = b.depth - a.depth;
        let t = if span > 0.0 { ((depth - a.depth) / span).clamp(0.0, 1.0) } else { 0.0 };
        Some(a.position.lerp(b.position, t))
    }

    /// The straight pieces the trace is drawn in between depths `from` and
    /// `to`, clamped to the trace: cut at every station, at the ends of
    /// `render_ranges`, and at `cuts`. A piece whose middle lies outside
    /// every render range is not drawn and is left out, as is one with no
    /// length. Every builder of a hole's geometry and the pick walk the
    /// trace through this, so what is clickable is what is drawn.
    pub(crate) fn trace_pieces(&self, from: f64, to: f64, cuts: &[f64]) -> Vec<TracePiece> {
        let (Some(first), Some(last)) = (self.trace.first(), self.trace.last()) else {
            return Vec::new();
        };
        let (from, to) = (from.max(first.depth), to.min(last.depth));
        if !from.is_finite() || !to.is_finite() || to <= from {
            return Vec::new();
        }
        let inside = |depth: &f64| *depth > from && *depth < to;
        let mut boundaries: Vec<f64> = self
            .trace
            .iter()
            .map(|station| station.depth)
            .filter(inside)
            .chain(self.render_ranges.iter().flat_map(|&(start, end)| [start, end]).filter(inside))
            .chain(cuts.iter().copied().filter(inside))
            .chain([from, to])
            .collect();
        boundaries.sort_by(f64::total_cmp);
        boundaries.dedup_by(|a, b| (*a - *b).abs() <= 1.0e-9);

        let mut pieces = Vec::with_capacity(boundaries.len().saturating_sub(1));
        let mut previous = self.position_at_depth(boundaries[0]);
        for pair in boundaries.windows(2) {
            let start = previous;
            let end = self.position_at_depth(pair[1]);
            previous = end;
            if pair[1] <= pair[0] + 1.0e-9 {
                continue;
            }
            let (Some(start), Some(end)) = (start, end) else {
                continue;
            };
            if start.distance_squared(end) <= 1.0e-18 {
                continue;
            }
            let midpoint = (pair[0] + pair[1]) * 0.5;
            if !self.render_ranges.is_empty() && !self.render_ranges.iter().any(|(start, end)| *start <= midpoint && midpoint < *end) {
                continue;
            }
            pieces.push(TracePiece {
                from: pair[0],
                to: pair[1],
                start,
                end,
            });
        }
        pieces
    }
}

/// One straight piece of a drawn trace: its depths and where they stand.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TracePiece {
    pub(crate) from: f64,
    pub(crate) to: f64,
    pub(crate) start: DVec3,
    pub(crate) end: DVec3,
}

fn trace_box(collar: DVec3, trace: &[TraceStation]) -> WorldBox {
    let mut min = collar;
    let mut max = collar;
    for station in trace {
        min = min.min(station.position);
        max = max.max(station.position);
    }
    (min, max)
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum DrillFieldKind {
    Numeric { min: f64, max: f64 },
    Categorical { categories: Vec<String> },
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DrillField {
    pub(crate) key: String,
    pub(crate) label: String,
    pub(crate) kind: DrillFieldKind,
}

#[derive(Clone, Debug)]
pub(crate) struct DrillHoleDataset {
    pub(crate) holes: Vec<DrillHole>,
    pub(crate) fields: Vec<DrillField>,
    pub(crate) bounds: Option<(DVec3, DVec3)>,
    /// One box per hole, in `holes` order, worked out in the same pass as
    /// `bounds` so a pick can test a hole before walking it. Not saved.
    pub(crate) hole_boxes: Vec<WorldBox>,
    /// The surface connectors tying the pattern in. Content rather than
    /// styling: they dirty the project and are undone with everything else.
    pub(crate) ties: Vec<TieIn>,
    /// Holes the round can start at. One initiation per collar, with any
    /// number of collars participating in the same firing graph.
    pub(crate) initiations: Vec<Initiation>,
}

impl DrillHoleDataset {
    pub(crate) fn new(mut holes: Vec<DrillHole>) -> Self {
        holes.sort_by(|a, b| crate::natural_sort::natural_cmp(&a.dhid, &b.dhid));
        // position_at_depth binary-searches on depth, so a trace must ascend.
        for hole in &mut holes {
            hole.trace.sort_by(|a, b| a.depth.total_cmp(&b.depth));
        }
        let fields = collect_fields(&holes);
        let mut dataset = Self {
            holes,
            fields,
            bounds: None,
            hole_boxes: Vec::new(),
            ties: Vec::new(),
            initiations: Vec::new(),
        };
        // The one gate every importer and project load passes through, so the
        // boxes cannot fall out of step with the traces.
        dataset.refresh_bounds();
        dataset
    }

    /// An absent box is safe: the caller walks the hole. A stale one is not:
    /// the gate says no, so hole geometry changes must end in `refresh_bounds`.
    pub(crate) fn hole_box(&self, index: usize) -> Option<WorldBox> {
        self.hole_boxes.get(index).copied()
    }

    /// The connector between two holes, whichever way round it runs.
    pub(crate) fn tie_between(&self, from: usize, to: usize) -> Option<&TieIn> {
        self.ties.iter().find(|tie| tie.joins(from, to))
    }

    /// When each hole fires, in milliseconds from the shot going off, or
    /// `None` for a hole no signal reaches.
    ///
    /// A hole fires on the *first* signal to arrive, so this is a multi-source
    /// shortest path from the initiation points rather than a walk of the graph: a round
    /// tied in a loop is well defined, and a connector that arrives after its
    /// hole has already gone simply does nothing.
    pub(crate) fn firing_times(&self) -> Vec<Option<u32>> {
        let mut times = vec![None; self.holes.len()];
        let mut queue = std::collections::BinaryHeap::new();
        for initiation in self.initiations.iter().filter(|initiation| initiation.hole < self.holes.len()) {
            if times[initiation.hole].is_none_or(|existing| initiation.delay_ms < existing) {
                times[initiation.hole] = Some(initiation.delay_ms);
                queue.push(std::cmp::Reverse((initiation.delay_ms, initiation.hole)));
            }
        }
        while let Some(std::cmp::Reverse((time, hole))) = queue.pop() {
            if times[hole].is_some_and(|settled| settled < time) {
                continue;
            }
            for tie in self.ties.iter().filter(|tie| tie.from == hole) {
                let Some(arrival) = times.get_mut(tie.to) else {
                    continue;
                };
                let candidate = time.saturating_add(tie.delay_ms);
                if arrival.is_none_or(|existing| candidate < existing) {
                    *arrival = Some(candidate);
                    queue.push(std::cmp::Reverse((candidate, tie.to)));
                }
            }
        }
        times
    }

    /// The ties as a file holds them, keyed by hole name.
    pub(crate) fn stored_ties(&self) -> StoredTieIns {
        let name = |index: usize| self.holes.get(index).map(|hole| hole.dhid.clone());
        StoredTieIns {
            ties: self
                .ties
                .iter()
                .filter_map(|tie| {
                    Some(StoredTieIn {
                        from: name(tie.from)?,
                        to: name(tie.to)?,
                        delay_ms: tie.delay_ms,
                        product: tie.product.clone(),
                        color: tie.color,
                    })
                })
                .collect(),
            initiations: self
                .initiations
                .iter()
                .filter_map(|initiation| {
                    Some(StoredInitiation {
                        hole: name(initiation.hole)?,
                        delay_ms: initiation.delay_ms,
                    })
                })
                .collect(),
            initiation: None,
        }
    }

    /// Resolve stored ties back onto this dataset's holes, reporting how many
    /// were dropped because a hole they named is no longer here. Called after
    /// construction, since it is [`Self::new`] that fixes the hole order the
    /// indices are against.
    pub(crate) fn apply_stored_ties(&mut self, stored: StoredTieIns) -> usize {
        let index_of = |name: &str| self.holes.iter().position(|hole| hole.dhid == name);
        let mut dropped = 0;
        self.ties = stored
            .ties
            .into_iter()
            .filter_map(|tie| {
                let (Some(from), Some(to)) = (index_of(&tie.from), index_of(&tie.to)) else {
                    dropped += 1;
                    return None;
                };
                Some(TieIn {
                    from,
                    to,
                    delay_ms: tie.delay_ms,
                    product: tie.product,
                    color: tie.color,
                })
            })
            .collect();
        let stored_initiations = stored.initiations.into_iter().chain(stored.initiation);
        self.initiations = stored_initiations
            .filter_map(|initiation| {
                let Some(hole) = index_of(&initiation.hole) else {
                    dropped += 1;
                    return None;
                };
                Some(Initiation {
                    hole,
                    delay_ms: initiation.delay_ms,
                })
            })
            .collect();
        // A collar has one editable initiation card. If a malformed file
        // names it twice, keep the last value rather than drawing stacked
        // cards or feeding duplicate sources into the firing graph.
        self.initiations.reverse();
        let mut seen = std::collections::HashSet::new();
        self.initiations.retain(|initiation| seen.insert(initiation.hole));
        self.initiations.reverse();
        dropped
    }

    /// Approximate retained size, used by the undo history's memory budget.
    pub(crate) fn estimated_bytes(&self) -> usize {
        size_of::<Self>()
            + self
                .holes
                .iter()
                .map(|hole| {
                    size_of::<DrillHole>()
                        + hole.dhid.len()
                        + hole.trace.len() * size_of::<TraceStation>()
                        + hole.render_ranges.len() * size_of::<(f64, f64)>()
                        + hole
                            .intervals
                            .iter()
                            .map(|interval| {
                                size_of::<DrillInterval>()
                                    + interval
                                        .values
                                        .iter()
                                        .map(|(key, value)| {
                                            key.len()
                                                + size_of::<DrillValue>()
                                                + match value {
                                                    DrillValue::Category(text) => text.len(),
                                                    DrillValue::Numeric(_) => 0,
                                                }
                                        })
                                        .fold(0usize, usize::saturating_add)
                                    + interval.logged.as_ref().map_or(0, |logged| {
                                        logged
                                            .values
                                            .iter()
                                            .map(|(key, value)| {
                                                key.len()
                                                    + size_of::<DrillValue>()
                                                    + match value {
                                                        DrillValue::Category(text) => text.len(),
                                                        DrillValue::Numeric(_) => 0,
                                                    }
                                            })
                                            .fold(0usize, usize::saturating_add)
                                    })
                            })
                            .fold(0usize, usize::saturating_add)
                })
                .fold(0usize, usize::saturating_add)
            + self.hole_boxes.len() * size_of::<WorldBox>()
            + self.ties.iter().map(|tie| size_of::<TieIn>() + tie.product.len()).fold(0usize, usize::saturating_add)
            + self.initiations.len() * size_of::<Initiation>()
            + self
                .fields
                .iter()
                .map(|field| {
                    size_of::<DrillField>()
                        + field.key.len()
                        + field.label.len()
                        + match &field.kind {
                            DrillFieldKind::Categorical { categories } => categories.iter().map(|value| size_of::<String>() + value.len()).fold(0usize, usize::saturating_add),
                            DrillFieldKind::Numeric { .. } => 0,
                        }
                })
                .fold(0usize, usize::saturating_add)
    }

    pub(crate) fn field(&self, key: &str) -> Option<&DrillField> {
        self.fields.iter().find(|field| field.key == key)
    }

    pub(crate) fn refresh_bounds(&mut self) {
        let (bounds, hole_boxes) = drill_extents(&self.holes);
        self.bounds = bounds;
        self.hole_boxes = hole_boxes;
        debug_assert_eq!(self.hole_boxes.len(), self.holes.len());
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum DrillColorPreset {
    Rainbow,
    Grayscale,
    Heat,
    GreenYellowRed,
}

impl DrillColorPreset {
    pub(crate) const ALL: [Self; 4] = [Self::Rainbow, Self::Grayscale, Self::Heat, Self::GreenYellowRed];

    pub(crate) fn label(self) -> String {
        match self {
            Self::Rainbow => crate::i18n::tr!(literal = "Rainbow"),
            Self::Grayscale => crate::i18n::tr!(literal = "Grayscale"),
            Self::Heat => crate::i18n::tr!(literal = "Heat"),
            Self::GreenYellowRed => crate::i18n::tr!(literal = "Green–Yellow–Red"),
        }
    }

    pub(crate) fn smooth(self) -> bool {
        matches!(self, Self::Rainbow | Self::Grayscale | Self::Heat)
    }

    pub(crate) fn stops(self) -> Vec<DrillColorStop> {
        let colors: &[[f32; 3]] = match self {
            Self::Rainbow => &[[0.05, 0.20, 0.95], [0.00, 0.85, 0.95], [0.05, 0.82, 0.20], [1.00, 0.85, 0.00], [0.92, 0.02, 0.02]],
            Self::Grayscale => &[[0.05, 0.05, 0.05], [0.95, 0.95, 0.95]],
            Self::Heat => &[[0.02, 0.02, 0.02], [0.85, 0.00, 0.00], [1.00, 0.78, 0.00], [1.00, 1.00, 0.92]],
            Self::GreenYellowRed => &[[0.00, 0.82, 0.20], [1.00, 0.85, 0.00], [0.92, 0.00, 0.00]],
        };
        let denom = if self.smooth() { colors.len().saturating_sub(1).max(1) } else { colors.len().max(1) } as f32;
        colors
            .iter()
            .enumerate()
            .map(|(index, color)| DrillColorStop {
                t: index as f32 / denom,
                color: *color,
            })
            .collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct DrillColorStop {
    pub(crate) t: f32,
    pub(crate) color: [f32; 3],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct DrillCategoryColor {
    pub(crate) value: String,
    pub(crate) color: [f32; 3],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(from = "Vec<DrillCategoryColor>", into = "Vec<DrillCategoryColor>")]
pub(crate) struct CategoryTable {
    entries: Vec<DrillCategoryColor>,
    hash: u64,
}

impl CategoryTable {
    fn new(mut entries: Vec<DrillCategoryColor>) -> Self {
        entries.sort_by(|a, b| a.value.cmp(&b.value));
        entries.dedup_by(|a, b| a.value == b.value);
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        for entry in &entries {
            entry.value.hash(&mut hasher);
            for channel in entry.color {
                channel.to_bits().hash(&mut hasher);
            }
        }
        Self { entries, hash: hasher.finish() }
    }

    pub(crate) fn content_hash(&self) -> u64 {
        self.hash
    }
}

impl From<Vec<DrillCategoryColor>> for CategoryTable {
    fn from(entries: Vec<DrillCategoryColor>) -> Self {
        Self::new(entries)
    }
}

impl From<CategoryTable> for Vec<DrillCategoryColor> {
    fn from(table: CategoryTable) -> Self {
        table.entries
    }
}

impl Default for CategoryTable {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl std::ops::Deref for CategoryTable {
    type Target = [DrillCategoryColor];
    fn deref(&self) -> &Self::Target {
        &self.entries
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct DrillColorState {
    pub(crate) active_field: Option<String>,
    pub(crate) preset: DrillColorPreset,
    pub(crate) smooth: bool,
    pub(crate) stops: Vec<DrillColorStop>,
    pub(crate) categories: CategoryTable,
    /// Multiplies the drilled diameter where a hole is drawn: at true width a
    /// hole reads as a pipe beside the geology and a set of thousands as a
    /// mat, so how wide a set draws is the set's to choose.
    #[serde(default = "default_radius_scale", deserialize_with = "clamped_radius_scale")]
    pub(crate) radius_scale: f64,
    /// Narrowest a hole is drawn, whatever the scale above and however far
    /// the eye is: below this it would flicker out rather than thin.
    #[serde(default = "default_min_pixel_diameter", deserialize_with = "clamped_min_pixel_diameter")]
    pub(crate) min_pixel_diameter: f32,
    /// Named groups of codes mined together, kept per source field. They
    /// travel with the colour state so a project written before them loads
    /// with none.
    #[serde(default, deserialize_with = "lenient_working_sections")]
    pub(crate) working_sections: Vec<WorkingSection>,
    /// Colour the active field by working section: a code in a group takes
    /// the group's colour, any other code keeps its own.
    #[serde(default)]
    pub(crate) by_working_section: bool,
    /// How the set is drawn. A set saved before there was a choice reads
    /// back at its true diameter, the look it was saved with; an imported
    /// set starts as string and discs (see `Self::for_logged_holes`).
    #[serde(default = "saved_before_styles_hole_style", deserialize_with = "lenient_hole_style")]
    pub(crate) hole_style: DrillHoleStyle,
    /// The discs' diameter in metres, drawn string and discs.
    #[serde(default = "default_disc_diameter", deserialize_with = "clamped_disc_diameter")]
    pub(crate) disc_diameter: f64,
    /// The string's width on screen in pixels, drawn string and discs: it
    /// neither thins nor thickens with zoom.
    #[serde(default = "default_string_pixel_width", deserialize_with = "clamped_string_pixel_width")]
    pub(crate) string_pixel_width: f32,
}

/// How a drill hole set is drawn in the 3D view.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) enum DrillHoleStyle {
    /// A cylinder at the drilled diameter times the set's width scale.
    TrueDiameter,
    /// A thin string of constant screen width along the trace, and a disc of
    /// a chosen diameter on every interval with a value in the colour field.
    StringAndDiscs,
}

impl DrillHoleStyle {
    pub(crate) const ALL: [Self; 2] = [Self::TrueDiameter, Self::StringAndDiscs];

    pub(crate) fn label(self) -> String {
        match self {
            Self::TrueDiameter => tr!(literal = "True diameter"),
            Self::StringAndDiscs => tr!(literal = "String and discs"),
        }
    }
}

/// Seams or plies of one categorical field that are mined as one unit.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) struct WorkingSection {
    pub(crate) name: String,
    pub(crate) field: String,
    pub(crate) codes: Vec<String>,
}

/// Why a working section cannot be kept as named.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SectionProblem {
    /// Blank once trimmed.
    NoName,
    /// A code of the field has the name, ignoring case: a pick or a colour
    /// under it could mean either.
    NameIsCode,
    /// Another section of the field has the name, ignoring case.
    DuplicateName,
    /// Every code it lists is blank or already in an earlier section of the
    /// field.
    NoCodes,
}

impl SectionProblem {
    pub(crate) fn message(self) -> String {
        match self {
            Self::NoName => tr!(literal = "A working section needs a name."),
            Self::NameIsCode => tr!(literal = "A code of this field already has that name."),
            Self::DuplicateName => tr!(literal = "Another working section of this field has that name."),
            Self::NoCodes => tr!(literal = "Every code it lists is already in another working section."),
        }
    }
}

/// A section [`tidy_working_sections`] left out, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DroppedSection {
    pub(crate) name: String,
    pub(crate) problem: SectionProblem,
}

/// Case-blind comparison of two names, without allocating.
fn same_name(a: &str, b: &str) -> bool {
    a.chars().flat_map(char::to_lowercase).eq(b.chars().flat_map(char::to_lowercase))
}

/// What stops `name` naming a new working section of `field`, given the
/// field's codes and the sections already kept. The editor asks this before
/// it offers to add one, and [`tidy_working_sections`] before it keeps one,
/// so what the one allows the other keeps.
pub(crate) fn working_section_name_problem(name: &str, field: &str, codes: &[String], sections: &[WorkingSection]) -> Option<SectionProblem> {
    let name = name.trim();
    if name.is_empty() {
        Some(SectionProblem::NoName)
    } else if codes.iter().any(|code| same_name(code, name)) {
        Some(SectionProblem::NameIsCode)
    } else if sections.iter().any(|section| section.field == field && same_name(&section.name, name)) {
        Some(SectionProblem::DuplicateName)
    } else {
        None
    }
}

/// The codes `fields` lists for `key`, or none for a field that is not
/// categorical or not there.
fn field_codes<'a>(fields: &'a [DrillField], key: &str) -> &'a [String] {
    match fields.iter().find(|field| field.key == key).map(|field| &field.kind) {
        Some(DrillFieldKind::Categorical { categories }) => categories,
        _ => &[],
    }
}

/// A list of working sections as the app keeps it: names trimmed, each code
/// in at most one section per field (the first that claims it), and no
/// section kept whose name is blank, is a code of its field, or repeats
/// another's, ignoring case. What is left out comes back with the reason.
/// A section of a field `fields` does not hold is kept as it is, as a colour
/// for a code a shorter extract lacks is.
pub(crate) fn tidy_working_sections(sections: Vec<WorkingSection>, fields: &[DrillField]) -> (Vec<WorkingSection>, Vec<DroppedSection>) {
    let mut kept: Vec<WorkingSection> = Vec::new();
    let mut dropped = Vec::new();
    for mut section in sections {
        section.name = section.name.trim().to_owned();
        if let Some(problem) = working_section_name_problem(&section.name, &section.field, field_codes(fields, &section.field), &kept) {
            dropped.push(DroppedSection { name: section.name, problem });
            continue;
        }
        let mut codes: Vec<String> = Vec::new();
        for code in section.codes {
            let claimed = kept.iter().any(|other| other.field == section.field && other.codes.contains(&code));
            if !code.trim().is_empty() && !claimed && !codes.contains(&code) {
                codes.push(code);
            }
        }
        if codes.is_empty() {
            dropped.push(DroppedSection {
                name: section.name,
                problem: SectionProblem::NoCodes,
            });
            continue;
        }
        section.codes = codes;
        kept.push(section);
    }
    (kept, dropped)
}

/// Reads the working sections an entry at a time, so one malformed entry,
/// or a value that is not a list at all, costs only itself and not the
/// colours saved beside it. [`skipped_working_sections`] counts what this
/// passes over, for the loader to report.
fn lenient_working_sections<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Vec<WorkingSection>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Entry {
        Section(WorkingSection),
        Unreadable(serde::de::IgnoredAny),
    }
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum List {
        Entries(Vec<Entry>),
        Unreadable(serde::de::IgnoredAny),
    }
    Ok(match List::deserialize(deserializer)? {
        List::Entries(entries) => entries
            .into_iter()
            .filter_map(|entry| match entry {
                Entry::Section(section) => Some(section),
                Entry::Unreadable(_) => None,
            })
            .collect(),
        List::Unreadable(_) => Vec::new(),
    })
}

/// How many saved working sections the reader passes over in a colour
/// state's JSON: each list entry that is not a section, or one for a value
/// that is not a list. Nothing saved counts nothing.
pub(crate) fn skipped_working_sections(color: &serde_json::Value) -> usize {
    match color.get("working_sections") {
        None | Some(serde_json::Value::Null) => 0,
        Some(serde_json::Value::Array(entries)) => entries.iter().filter(|entry| WorkingSection::deserialize(*entry).is_err()).count(),
        Some(_) => 1,
    }
}

/// A dataset is drawn at its drilled width until someone says otherwise.
pub(crate) fn default_radius_scale() -> f64 {
    1.0
}

fn default_min_pixel_diameter() -> f32 {
    MIN_RENDER_PIXEL_DIAMETER
}

/// What the width controls accept: a hole thinner than a twentieth of its
/// drilled width is a line, and one twice it is a shaft.
pub(crate) const RADIUS_SCALE_RANGE: std::ops::RangeInclusive<f64> = 0.05..=2.0;
/// One pixel is the thinnest a hole can be and still be drawn at all.
pub(crate) const MIN_PIXEL_DIAMETER_RANGE: std::ops::RangeInclusive<f32> = 1.0..=8.0;

// Clamped where a value comes in, not only where the dialog sets one: a file
// edited by hand must not draw a hole at nothing and then save that back.
fn clamped_radius_scale<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<f64, D::Error> {
    let value = f64::deserialize(deserializer)?;
    Ok(if value.is_finite() {
        value.clamp(*RADIUS_SCALE_RANGE.start(), *RADIUS_SCALE_RANGE.end())
    } else {
        default_radius_scale()
    })
}

fn clamped_min_pixel_diameter<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<f32, D::Error> {
    let value = f32::deserialize(deserializer)?;
    Ok(if value.is_finite() {
        value.clamp(*MIN_PIXEL_DIAMETER_RANGE.start(), *MIN_PIXEL_DIAMETER_RANGE.end())
    } else {
        default_min_pixel_diameter()
    })
}

/// The style of a set saved before there was one: drawn at its true
/// diameter, as it was when saved. Deliberately not what a new set gets.
fn saved_before_styles_hole_style() -> DrillHoleStyle {
    DrillHoleStyle::TrueDiameter
}

/// A style this build does not know, from a newer one, reads as the saved
/// look rather than costing the set every colour saved beside it.
fn lenient_hole_style<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<DrillHoleStyle, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Style {
        Known(DrillHoleStyle),
        Unknown(serde::de::IgnoredAny),
    }
    Ok(match Style::deserialize(deserializer)? {
        Style::Known(style) => style,
        Style::Unknown(_) => saved_before_styles_hole_style(),
    })
}

pub(crate) fn default_disc_diameter() -> f64 {
    DEFAULT_DISC_DIAMETER
}

pub(crate) fn default_string_pixel_width() -> f32 {
    DEFAULT_STRING_PIXEL_WIDTH
}

/// The disc diameter as the controls and a saved file may set it: a value
/// that is not a number is the default, anything else is held in range.
pub(crate) fn clamp_disc_diameter(value: f64) -> f64 {
    if value.is_finite() {
        value.clamp(*DISC_DIAMETER_RANGE.start(), *DISC_DIAMETER_RANGE.end())
    } else {
        default_disc_diameter()
    }
}

/// The string width as the controls and a saved file may set it.
pub(crate) fn clamp_string_pixel_width(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(*STRING_PIXEL_WIDTH_RANGE.start(), *STRING_PIXEL_WIDTH_RANGE.end())
    } else {
        default_string_pixel_width()
    }
}

fn clamped_disc_diameter<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<f64, D::Error> {
    Ok(clamp_disc_diameter(f64::deserialize(deserializer)?))
}

fn clamped_string_pixel_width<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<f32, D::Error> {
    Ok(clamp_string_pixel_width(f32::deserialize(deserializer)?))
}

impl Default for DrillColorState {
    fn default() -> Self {
        let preset = DrillColorPreset::Rainbow;
        Self {
            active_field: None,
            preset,
            smooth: preset.smooth(),
            stops: preset.stops(),
            categories: CategoryTable::default(),
            radius_scale: default_radius_scale(),
            min_pixel_diameter: default_min_pixel_diameter(),
            working_sections: Vec::new(),
            by_working_section: false,
            // The look a set had before there was a choice. A new set takes
            // the constructor for where it came from instead.
            hole_style: saved_before_styles_hole_style(),
            disc_diameter: default_disc_diameter(),
            string_pixel_width: default_string_pixel_width(),
        }
    }
}

impl DrillColorState {
    /// A set of logged holes, imported: drawn as string and discs, the
    /// picture a modeller reads intervals by.
    pub(crate) fn for_logged_holes() -> Self {
        Self {
            hole_style: DrillHoleStyle::StringAndDiscs,
            ..Self::default()
        }
    }

    /// A set of planned holes, laid out as a pattern: drawn at their true
    /// diameter. They carry no intervals to put discs on, and their design
    /// diameter, and the tie-ins sized from it, must read as drawn.
    pub(crate) fn for_planned_holes() -> Self {
        Self {
            hole_style: DrillHoleStyle::TrueDiameter,
            ..Self::default()
        }
    }

    /// The narrowest a disc is drawn on screen, in pixels: always wider than
    /// the string it sits on, so its colour shows either side.
    pub(crate) fn disc_min_pixel_diameter(&self) -> f32 {
        self.string_pixel_width + DISC_PIXEL_MARGIN
    }

    /// The colour chosen for one code, by binary search over the sorted table.
    pub(crate) fn category_color(&self, value: &str) -> Option<[f32; 3]> {
        self.categories
            .binary_search_by(|entry| entry.value.as_str().cmp(value))
            .ok()
            .map(|index| self.categories[index].color)
    }

    /// The working section of `field` that holds `code`, if any.
    pub(crate) fn working_section_of(&self, field: &str, code: &str) -> Option<&WorkingSection> {
        self.working_sections
            .iter()
            .find(|section| section.field == field && section.codes.iter().any(|member| member == code))
    }

    /// The working section of `field` called `name`, if any.
    pub(crate) fn working_section_named(&self, field: &str, name: &str) -> Option<&WorkingSection> {
        self.working_sections.iter().find(|section| section.field == field && section.name == name)
    }

    /// The section each code is coloured as, gathered once for a rebuild;
    /// empty unless colouring the active field by working section.
    pub(crate) fn section_lookup(&self) -> SectionLookup<'_> {
        let mut names = HashMap::new();
        if self.by_working_section
            && let Some(field) = self.active_field.as_deref()
        {
            for section in self.working_sections.iter().filter(|section| section.field == field) {
                for code in &section.codes {
                    names.entry(code.as_str()).or_insert(section.name.as_str());
                }
            }
        }
        SectionLookup { names }
    }

    /// The code an interval is coloured as: its working section's name when
    /// colouring by section and the code is in one, otherwise the code. This
    /// walks the section list; a pass over many intervals takes a
    /// [`Self::section_lookup`] instead.
    pub(crate) fn display_code<'a>(&'a self, code: &'a str) -> &'a str {
        if !self.by_working_section {
            return code;
        }
        self.active_field
            .as_deref()
            .and_then(|field| self.working_section_of(field, code))
            .map_or(code, |section| section.name.as_str())
    }

    /// What the colour table lists for `field` coloured by section: every
    /// section name, then each code no section holds, in the field's order.
    pub(crate) fn working_section_view(&self, field: &str, categories: &[String]) -> Vec<String> {
        let mut view: Vec<String> = self
            .working_sections
            .iter()
            .filter(|section| section.field == field)
            .map(|section| section.name.clone())
            .collect();
        for code in categories {
            if self.working_section_of(field, code).is_none() && !view.contains(code) {
                view.push(code.clone());
            }
        }
        view
    }

    /// Every write goes through here so the order the lookup searches holds.
    pub(crate) fn set_categories(&mut self, categories: Vec<DrillCategoryColor>) {
        self.categories = CategoryTable::new(categories);
    }

    /// Give every code in `field`, and every working section of it, a
    /// colour, keeping every colour already chosen, and return how many of
    /// the field's codes were filled in; section names are not counted.
    /// Entries for codes absent from `field` stay: a shorter extract must not
    /// lose the full one's picks.
    pub(crate) fn reconcile_categories(&mut self, field: &DrillField) -> usize {
        let DrillFieldKind::Categorical { categories } = &field.kind else {
            return 0;
        };
        let added = self.fill_category_colors(categories);
        if self.working_sections.iter().any(|section| section.field == field.key) {
            self.fill_category_colors(&self.working_section_view(&field.key, categories));
        }
        added
    }

    /// Give each of `categories` a colour where it has none, avoiding the
    /// colours the others already hold, and return how many were filled in.
    pub(crate) fn fill_category_colors(&mut self, categories: &[String]) -> usize {
        let mut table = Vec::from(std::mem::take(&mut self.categories));
        let mut used: Vec<[f32; 3]> = Vec::new();
        for value in categories {
            if let Ok(index) = table.binary_search_by(|entry| entry.value.as_str().cmp(value)) {
                used.push(table[index].color);
            }
        }
        let mut filled = Vec::new();
        for (index, value) in categories.iter().enumerate() {
            if table.binary_search_by(|entry| entry.value.as_str().cmp(value)).is_err() {
                let mut color = None;
                for offset in 0..=used.len() {
                    let candidate = generated_category_color(index + offset);
                    if !used.contains(&candidate) {
                        color = Some(candidate);
                        break;
                    }
                }
                let color = color.unwrap_or_else(|| generated_category_color(index));
                used.push(color);
                filled.push(DrillCategoryColor { value: value.clone(), color });
            }
        }
        let added = filled.len();
        table.append(&mut filled);
        self.set_categories(table);
        added
    }
}

#[derive(Clone, Debug)]
pub(crate) struct LoadedDrillHoleDataset {
    pub(crate) name: String,
    pub(crate) source: DrillHoleSource,
    pub(crate) dataset: Arc<DrillHoleDataset>,
}

#[derive(Clone, Debug)]
pub(crate) struct OpenDrillHoleDataset {
    pub(crate) id: DrillHoleId,
    pub(crate) state: ProjectItemState,
    pub(crate) name: String,
    pub(crate) dataset: Arc<DrillHoleDataset>,
    pub(crate) color: DrillColorState,
}

impl OpenDrillHoleDataset {
    pub(crate) fn entity_id(&self) -> crate::model::SceneEntityId {
        crate::model::SceneEntityId::DrillHole(self.id)
    }
}

/// One hole inside a dataset.
///
/// A dataset is a scene entity - it is what the explorer lists, what is
/// hidden, frozen and coloured - so [`crate::model::SceneEntityId`] names the
/// dataset and stops there. A single hole is a part of one, the way a vertex
/// is part of a polyline, and this is how the Drill & Blast workspace points
/// at it: the dataset's id and the hole's index in
/// [`DrillHoleDataset::holes`], which is fixed for as long as the dataset is
/// loaded ([`DrillHoleDataset::new`] sorts the holes once, on import).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct DrillHoleRef {
    pub(crate) dataset: DrillHoleId,
    pub(crate) hole: usize,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SurveyObservation {
    pub(crate) depth: f64,
    pub(crate) azimuth: Option<f64>,
    pub(crate) dip: Option<f64>,
    pub(crate) position: Option<DVec3>,
}

/// A resolved trace and whether an observation steered it past the collar.
pub(crate) struct ResolvedTrace {
    pub(crate) stations: Vec<TraceStation>,
    pub(crate) steered: bool,
}

pub(crate) fn resolve_trace(collar: DVec3, observations: &mut [SurveyObservation], target_depth: f64) -> ResolvedTrace {
    observations.sort_by(|a, b| a.depth.total_cmp(&b.depth));
    let mut trace = vec![TraceStation { depth: 0.0, position: collar }];
    let mut last_orientation = observations.iter().find_map(SurveyObservation::orientation);
    let mut steered = false;
    for observation in observations.iter().copied() {
        if !observation.depth.is_finite() || observation.depth < 0.0 {
            continue;
        }
        let previous = *trace.last().expect("collar station exists");
        if observation.depth + 1.0e-9 < previous.depth {
            continue;
        }
        let stored_position = observation.position.filter(|position| position.is_finite());
        steered |= observation.orientation().is_some();
        let position = stored_position.unwrap_or_else(|| {
            let (azimuth, dip) = last_orientation.unwrap_or((0.0, -90.0));
            project_tangent(previous.position, observation.depth - previous.depth, azimuth, dip)
        });
        if observation.depth > previous.depth + 1.0e-9 {
            steered |= stored_position.is_some() || last_orientation.is_some();
            trace.push(TraceStation {
                depth: observation.depth,
                position,
            });
        } else if let Some(first) = trace.first_mut() {
            first.position = collar;
        }
        last_orientation = observation
            .orientation()
            .or_else(|| stored_position.and_then(|_| orientation_from_delta(position - previous.position)))
            .or(last_orientation);
    }
    let previous = *trace.last().expect("collar station exists");
    if target_depth.is_finite() && target_depth > previous.depth + 1.0e-9 {
        let (azimuth, dip) = last_orientation.unwrap_or((0.0, -90.0));
        steered |= last_orientation.is_some();
        trace.push(TraceStation {
            depth: target_depth,
            position: project_tangent(previous.position, target_depth - previous.depth, azimuth, dip),
        });
    }
    ResolvedTrace { stations: trace, steered }
}

impl SurveyObservation {
    fn orientation(&self) -> Option<(f64, f64)> {
        match (self.azimuth, self.dip) {
            (Some(azimuth), Some(dip)) if azimuth.is_finite() && dip.is_finite() => Some((azimuth, dip)),
            _ => None,
        }
    }
}

fn orientation_from_delta(delta: DVec3) -> Option<(f64, f64)> {
    let horizontal = delta.x.hypot(delta.y);
    let length = horizontal.hypot(delta.z);
    (length > 1.0e-12).then(|| (delta.x.atan2(delta.y).to_degrees().rem_euclid(360.0), delta.z.atan2(horizontal).to_degrees()))
}

fn project_tangent(origin: DVec3, distance: f64, azimuth_degrees: f64, dip_degrees: f64) -> DVec3 {
    let azimuth = azimuth_degrees.to_radians();
    let dip = dip_degrees.to_radians();
    let horizontal = distance * dip.cos();
    origin + DVec3::new(horizontal * azimuth.sin(), horizontal * azimuth.cos(), distance * dip.sin())
}

fn collect_fields(holes: &[DrillHole]) -> Vec<DrillField> {
    let mut numeric: BTreeMap<String, (String, f64, f64, bool)> = BTreeMap::new();
    let mut categorical: BTreeMap<String, (String, BTreeSet<String>)> = BTreeMap::new();
    for hole in holes {
        for interval in &hole.intervals {
            for (key, value) in &interval.values {
                let label = key.clone();
                match value {
                    // Sentinels stay out of the range; the field still shows.
                    DrillValue::Numeric(value) if value.is_finite() => {
                        let sentinel = crate::model::block_model::is_no_data_sentinel(*value);
                        numeric
                            .entry(key.clone())
                            .and_modify(|(_, min, max, has_real)| {
                                if sentinel {
                                    return;
                                }
                                if *has_real {
                                    *min = min.min(*value);
                                    *max = max.max(*value);
                                } else {
                                    *min = *value;
                                    *max = *value;
                                    *has_real = true;
                                }
                            })
                            .or_insert_with(|| if sentinel { (label, 0.0, 0.0, false) } else { (label, *value, *value, true) });
                    }
                    DrillValue::Category(value) if !value.trim().is_empty() => {
                        let values = &mut categorical.entry(key.clone()).or_insert_with(|| (label, BTreeSet::new())).1;
                        if !values.contains(value.as_str()) {
                            values.insert(value.clone());
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    let mut fields = Vec::new();
    fields.extend(numeric.into_iter().map(|(key, (label, min, max, _))| DrillField {
        key,
        label,
        kind: DrillFieldKind::Numeric { min, max },
    }));
    fields.extend(categorical.into_iter().map(|(key, (label, categories))| {
        // Every distinct code reaches the model; the ramp limit is not theirs.
        let mut categories = categories.into_iter().collect::<Vec<_>>();
        categories.sort_by(|a, b| crate::natural_sort::natural_cmp(a, b));
        DrillField {
            key,
            label,
            kind: DrillFieldKind::Categorical { categories },
        }
    }));
    fields.sort_by(|a, b| crate::natural_sort::natural_cmp(&a.label, &b.label));
    fields
}

pub(crate) type WorldBox = (DVec3, DVec3);

/// One walk of the stations, so the dataset bounds and the per-hole boxes
/// cannot come from different station sets.
fn drill_extents(holes: &[DrillHole]) -> (Option<WorldBox>, Vec<WorldBox>) {
    let mut min = DVec3::splat(f64::INFINITY);
    let mut max = DVec3::splat(f64::NEG_INFINITY);
    let mut any = false;
    let mut boxes = Vec::with_capacity(holes.len());
    for hole in holes {
        let hole_box = trace_box(hole.collar_position(), &hole.trace);
        boxes.push(hole_box);
        // The bounds cover the collar marker; the hole's box is geometry alone.
        let radius = hole.render_radius() * COLLAR_MARKER_RADIUS_SCALE;
        min = min.min(hole_box.0 - DVec3::splat(radius));
        max = max.max(hole_box.1 + DVec3::splat(radius));
        any = true;
    }
    (any.then_some((min, max)), boxes)
}

/// The default colour for the code at `index` in a field's code list,
/// generated so no code can fall past the end of a palette: hues advance by
/// the golden angle through nine saturation and lightness bands, so near
/// hues still differ in tone, and no band reaches white, the no-value colour.
pub(crate) fn generated_category_color(index: usize) -> [f32; 3] {
    const GOLDEN_STEP: f64 = 0.618_033_988_749_895;
    const HUE_ORIGIN: f64 = 0.58;
    const BANDS: [(f32, f32); 9] = [
        (0.60, 0.52),
        (0.90, 0.70),
        (0.45, 0.32),
        (0.75, 0.60),
        (0.98, 0.42),
        (0.55, 0.46),
        (0.80, 0.36),
        (0.70, 0.66),
        (0.50, 0.40),
    ];
    let hue = (HUE_ORIGIN + index as f64 * GOLDEN_STEP).rem_euclid(1.0) as f32;
    let (saturation, lightness) = BANDS[index % BANDS.len()];
    hsl_to_rgb(hue, saturation, lightness)
}

/// Plain HSL to sRGB, in the component convention the shader takes.
fn hsl_to_rgb(hue: f32, saturation: f32, lightness: f32) -> [f32; 3] {
    let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let sector = hue.rem_euclid(1.0) * 6.0;
    let second = chroma * (1.0 - (sector % 2.0 - 1.0).abs());
    let (red, green, blue) = match sector as u32 {
        0 => (chroma, second, 0.0),
        1 => (second, chroma, 0.0),
        2 => (0.0, chroma, second),
        3 => (0.0, second, chroma),
        4 => (second, 0.0, chroma),
        _ => (chroma, 0.0, second),
    };
    let base = lightness - chroma * 0.5;
    [red + base, green + base, blue + base]
}

/// A colour for every code in `categories`, sorted by code for the lookup.
pub(crate) fn default_category_colors(categories: &[String]) -> Vec<DrillCategoryColor> {
    let colors = categories
        .iter()
        .enumerate()
        .map(|(index, value)| DrillCategoryColor {
            value: value.clone(),
            color: generated_category_color(index),
        })
        .collect::<Vec<_>>();
    CategoryTable::new(colors).into()
}

/// The section each code of the active field is coloured as; see
/// [`DrillColorState::section_lookup`].
pub(crate) struct SectionLookup<'a> {
    names: HashMap<&'a str, &'a str>,
}

impl<'a> SectionLookup<'a> {
    /// `code`'s section name, or `code` where it is in none.
    pub(crate) fn display_code<'b>(&'b self, code: &'b str) -> &'b str
    where
        'a: 'b,
    {
        self.names.get(code).copied().unwrap_or(code)
    }
}

/// What a reference pick is made on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ReferenceTarget {
    /// One logged code.
    Code(String),
    /// Every code of the working section of this name.
    Section(String),
}

impl ReferenceTarget {
    pub(crate) fn name(&self) -> &str {
        match self {
            Self::Code(name) | Self::Section(name) => name,
        }
    }

    pub(crate) fn label(&self) -> String {
        match self {
            Self::Code(code) => code.clone(),
            Self::Section(name) => tr_format!(literal = "%name% (working section)", name = name.clone()),
        }
    }
}

/// Which boundary of a working section a reference surface is built from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ReferenceSide {
    #[default]
    Roof,
    Floor,
}

impl ReferenceSide {
    pub(crate) const ALL: [Self; 2] = [Self::Roof, Self::Floor];

    pub(crate) fn label(self) -> String {
        match self {
            Self::Roof => tr!(literal = "Roof"),
            Self::Floor => tr!(literal = "Floor"),
        }
    }
}

/// What one hole contributes to a reference surface for one working section.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ReferencePick {
    /// Down-hole depth of the chosen boundary, or `None` where the hole never
    /// holds the value.
    pub(crate) depth: Option<f64>,
    /// How many separate runs of the value the hole holds. Two or more is a
    /// possible fault repeat: the uppermost run is used and the hole flagged.
    pub(crate) runs: usize,
}

/// The pick a hole gives for `value` in `field`, on `side`.
///
/// Intervals carrying the value form runs. An interval with no value in that
/// field sits inside a run without breaking it, since a parting is still the
/// same working section, while a different value ends it. The roof is the
/// top of a run's first interval, the floor its deepest base; with more
/// than one run the uppermost is taken and `runs` says so.
///
/// `codes` is one code, or every code of a working section: consecutive
/// intervals holding any of them form a single run.
pub(crate) fn reference_pick(hole: &DrillHole, field: &str, codes: &[String], side: ReferenceSide) -> ReferencePick {
    let mut intervals: Vec<&DrillInterval> = hole.intervals.iter().collect();
    intervals.sort_by(|a, b| a.from.total_cmp(&b.from));
    let mut runs: Vec<(f64, f64)> = Vec::new();
    let mut open: Option<(f64, f64)> = None;
    for interval in intervals {
        match interval.values.get(field) {
            Some(DrillValue::Category(code)) if codes.contains(code) => {
                // A nested or duplicated row sorts after the interval that
                // contains it, so the base is the deepest `to` in the run.
                open = Some(match open {
                    Some((top, base)) => (top, base.max(interval.to)),
                    None => (interval.from, interval.to),
                });
            }
            Some(DrillValue::Category(code)) if code.trim().is_empty() => {}
            None => {}
            Some(_) => {
                if let Some(run) = open.take() {
                    runs.push(run);
                }
            }
        }
    }
    if let Some(run) = open {
        runs.push(run);
    }
    let depth = runs.first().map(|&(top, base)| match side {
        ReferenceSide::Roof => top,
        ReferenceSide::Floor => base,
    });
    ReferencePick { depth, runs: runs.len() }
}

/// One disc of a hole drawn as string and discs: a stretch of the trace and
/// the interval whose value colours it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct HoleDisc {
    pub(crate) from: f64,
    pub(crate) to: f64,
    /// Index into the hole's `intervals`.
    pub(crate) interval: usize,
}

/// Whether a value earns its interval a disc. A blank code, or a number that
/// is not one or is a no-data marker, is nothing at all - the colour path
/// draws it white for the same reason - so that interval shows only string.
pub(crate) fn disc_value(value: &DrillValue) -> bool {
    match value {
        DrillValue::Category(code) => !code.trim().is_empty(),
        DrillValue::Numeric(number) => number.is_finite() && !crate::model::block_model::is_no_data_sentinel(*number),
    }
}

/// The discs `hole` shows for the colour field `field`, shallowest first,
/// clamped to the trace and to `render_ranges`.
///
/// Where intervals holding a value overlap - a seam logged whole and again
/// as its plies - the shortest one covering a depth is drawn there, ties to
/// the first logged: the plies show, and the whole seam fills only what
/// they leave. Discs therefore never overlap one another, so no two of them
/// fight for the same pixels.
pub(crate) fn hole_discs(hole: &DrillHole, field: &str) -> Vec<HoleDisc> {
    let (Some(first), Some(last)) = (hole.trace.first(), hole.trace.last()) else {
        return Vec::new();
    };
    let (min_depth, max_depth) = (first.depth, last.depth);
    // Also refuses a depth that is not a number.
    if min_depth.partial_cmp(&max_depth) != Some(std::cmp::Ordering::Less) {
        return Vec::new();
    }
    let mut candidates: Vec<usize> = hole
        .intervals
        .iter()
        .enumerate()
        .filter(|(_, interval)| interval.from.is_finite() && interval.to.is_finite() && interval.from < interval.to)
        .filter(|(_, interval)| interval.values.get(field).is_some_and(disc_value))
        .map(|(index, _)| index)
        .collect();
    if candidates.is_empty() {
        return Vec::new();
    }
    let mut boundaries: Vec<f64> = candidates
        .iter()
        .flat_map(|&index| [hole.intervals[index].from, hole.intervals[index].to])
        .chain(hole.render_ranges.iter().flat_map(|&(from, to)| [from, to]))
        .filter(|depth| depth.is_finite())
        .map(|depth| depth.clamp(min_depth, max_depth))
        .collect();
    boundaries.sort_by(f64::total_cmp);
    boundaries.dedup_by(|a, b| (*a - *b).abs() <= 1.0e-9);

    candidates.sort_by(|&a, &b| hole.intervals[a].from.total_cmp(&hole.intervals[b].from).then(a.cmp(&b)));
    let length = |index: usize| hole.intervals[index].to - hole.intervals[index].from;
    let mut next = 0usize;
    let mut active: Vec<usize> = Vec::new();
    let mut discs: Vec<HoleDisc> = Vec::new();
    for pair in boundaries.windows(2) {
        let (from, to) = (pair[0], pair[1]);
        if to <= from + 1.0e-9 {
            continue;
        }
        let midpoint = (from + to) * 0.5;
        while next < candidates.len() && hole.intervals[candidates[next]].from <= midpoint {
            active.push(candidates[next]);
            next += 1;
        }
        active.retain(|&index| midpoint < hole.intervals[index].to);
        if !hole.render_ranges.is_empty() && !hole.render_ranges.iter().any(|(start, end)| *start <= midpoint && midpoint < *end) {
            continue;
        }
        let Some(winner) = active.iter().copied().min_by(|&a, &b| length(a).total_cmp(&length(b)).then(a.cmp(&b))) else {
            continue;
        };
        match discs.last_mut() {
            Some(disc) if disc.interval == winner && (disc.to - from).abs() <= 1.0e-9 => disc.to = to,
            _ => discs.push(HoleDisc { from, to, interval: winner }),
        }
    }
    discs
}
