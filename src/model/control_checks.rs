//! Build Surface's checks on its control strings, shared with Clean Strings
//! so the clean-up judges a string exactly as the build will: too short,
//! closed without the flag, turning back or crossing itself, two strings
//! meeting at two heights or running along each other, and two points of
//! the fit too close to be two at heights too far apart to be one.

use std::collections::HashMap;

use anyhow::Result;
use glam::{DVec2, DVec3};

use crate::{
    i18n::tr,
    model::{
        kernel::{self, SegSeg},
        rbf::{DEFAULT_SPACING, MERGE_DISTANCE, MERGE_HEIGHT},
    },
};

/// The fewest vertices a control string is usable from: two make a segment,
/// and a segment is what the surface is held along.
pub(crate) const MINIMUM_CONTROL_VERTICES: usize = 2;

/// How far apart two controls' elevations may be where they cross in plan and
/// still count as one height; two interpretations meeting, so tighter than
/// [`kernel::Z_TOL`].
pub(crate) const CONTROL_AGREEMENT: f64 = 0.01;

/// How many controls crossing or doubling back on themselves the refusal
/// names one by one before it counts the rest, as the override report does.
pub(crate) const SELF_SHAPE_LINES: usize = 20;

/// Control strings carry no name of their own, so a refusal names one by
/// where it sat in the selection, counting from one.
pub(crate) fn too_few_control_vertices(index: usize, count: usize) -> String {
    tr!(
        "cmd-reference-surface-control-string-index-has-count",
        index = (index + 1).to_string(),
        count = count.to_string(),
        minimum = MINIMUM_CONTROL_VERTICES.to_string()
    )
}

/// A number alone cannot be found among hundreds of selected strings, so the
/// self-shape refusals also give where in plan the string goes wrong.
pub(crate) fn control_self_crossing(index: usize, position: DVec2) -> String {
    tr!(
        "cmd-reference-surface-control-string-index-crosses-itself",
        index = (index + 1).to_string(),
        x = format!("{:.3}", position.x),
        y = format!("{:.3}", position.y)
    )
}

pub(crate) fn control_doubles_back(index: usize, position: DVec2) -> String {
    tr!(
        "cmd-reference-surface-control-string-index-doubles-back",
        index = (index + 1).to_string(),
        x = format!("{:.3}", position.x),
        y = format!("{:.3}", position.y)
    )
}

pub(crate) fn control_ends_where_it_starts(index: usize) -> String {
    tr!("cmd-reference-surface-control-string-index-ends-where", index = (index + 1).to_string())
}

/// How many points one segment enters between polls of the cancel flag.
const CANCEL_POLL_STEPS: usize = 4096;

/// Stop the build once the job it runs in has been cancelled.
pub(crate) fn stop_if_cancelled(cancelled: &dyn Fn() -> bool) -> Result<()> {
    anyhow::ensure!(!cancelled(), crate::app::jobs::Cancelled);
    Ok(())
}

/// Two controls reading one plan position at two heights: which two, where,
/// and how far apart, for the geologist to decide between. The string
/// selected first is named first, each height beside its own string.
pub(crate) fn controls_disagree(left: usize, right: usize, position: DVec2, low: f64, high: f64) -> String {
    let (left, right, low, high) = if left <= right { (left, right, low, high) } else { (right, left, high, low) };
    tr!(
        "cmd-reference-surface-control-strings-b-disagree-x",
        a = (left + 1).to_string(),
        b = (right + 1).to_string(),
        x = format!("{:.3}", position.x),
        y = format!("{:.3}", position.y),
        za = format!("{low:.2}"),
        zb = format!("{high:.2}"),
        difference = format!("{:.2}", (high - low).abs())
    )
}

/// Two controls sharing a stretch of plan rather than a point: every position
/// along it is claimed twice, which is a job of its own.
pub(crate) fn controls_along_each_other(left: usize, right: usize) -> String {
    let (left, right) = (left.min(right), left.max(right));
    tr!("cmd-reference-surface-control-strings-b-run-along", a = (left + 1).to_string(), b = (right + 1).to_string())
}

/// Two vertices of one control too close in plan to be two points, at
/// heights too far apart to be one.
pub(crate) fn control_vertices_disagree(index: usize, position: DVec2) -> String {
    tr!(
        "cmd-reference-surface-control-string-index-has-two",
        index = (index + 1).to_string(),
        distance = MERGE_DISTANCE.to_string(),
        x = format!("{:.3}", position.x),
        y = format!("{:.3}", position.y)
    )
}

/// The points the controls enter the fit as: where they cross, then each
/// control's vertices with its segments densified at the grid spacing in
/// between. A point within [`MERGE_DISTANCE`] of one already entered is the
/// same point and goes in once; every two points that close must agree on
/// their height, as [`PointsCheck`] judges them.
pub(crate) fn control_points(controls: &[Vec<DVec3>], names: &[usize], crossings: &[Crossing], cancelled: &dyn Fn() -> bool) -> Result<Vec<DVec3>> {
    control_points_spaced(controls, names, crossings, DEFAULT_SPACING, cancelled)
}

/// The most points one segment is cut into. At the grid spacing that is a
/// segment 500 km long, longer than any control or seam string, and the fit
/// takes far fewer points than that, so a segment needing more is a stray.
pub(crate) const MOST_PIECES: usize = 100_000;

/// How many parts a segment is cut into at `spacing` in plan; `None` when
/// that is more than [`MOST_PIECES`], before any point is made.
pub(crate) fn pieces_along(start: DVec3, end: DVec3, spacing: f64) -> Option<usize> {
    let pieces = (start.truncate().distance(end.truncate()) / spacing).ceil().max(1.0);
    (pieces <= MOST_PIECES as f64).then_some(pieces as usize)
}

/// [`control_points`] with the segments densified at `spacing` in plan;
/// an infinite spacing enters the crossings and the vertices alone.
pub(crate) fn control_points_spaced(controls: &[Vec<DVec3>], names: &[usize], crossings: &[Crossing], spacing: f64, cancelled: &dyn Fn() -> bool) -> Result<Vec<DVec3>> {
    let mut check = PointsCheck::default();
    for index in 0..crossings.len() {
        if let Err(earlier) = check.read_crossing(crossings, index) {
            let earlier = check.point(earlier).2.unwrap_or(index);
            anyhow::bail!("{}", crossings_disagree(crossings, earlier, index, names));
        }
    }
    for (index, control) in controls.iter().enumerate() {
        stop_if_cancelled(cancelled)?;
        let (segments, count) = segments_at(control, 0);
        enter_point(&mut check, crossings, control[0], index, &segments[..count], names)?;
        for (at, segment) in control.windows(2).enumerate() {
            stop_if_cancelled(cancelled)?;
            let (start, end) = (segment[0], segment[1]);
            let Some(pieces) = pieces_along(start, end, spacing) else {
                let length = start.truncate().distance(end.truncate());
                anyhow::bail!(
                    "{}",
                    tr!(
                        "cmd-reference-surface-control-string-index-has-segment",
                        index = (names[index] + 1).to_string(),
                        length = format!("{length:.0}"),
                        most = MOST_PIECES.to_string()
                    )
                );
            };
            for step in 1..pieces {
                // A stray segment kilometres long enters millions of points.
                if step % CANCEL_POLL_STEPS == 0 {
                    stop_if_cancelled(cancelled)?;
                }
                let point = start + (end - start) * step as f64 / pieces as f64;
                enter_point(&mut check, crossings, point, index, &[[start, end]], names)?;
            }
            let (segments, count) = segments_at(control, at + 1);
            enter_point(&mut check, crossings, end, index, &segments[..count], names)?;
        }
    }
    Ok(check.into_entered())
}

/// Read one point of control `index` into `check`, refused in the words the
/// build uses for two of a string's own points or for two strings.
fn enter_point(check: &mut PointsCheck, crossings: &[Crossing], point: DVec3, index: usize, segments: &[[DVec3; 2]], names: &[usize]) -> Result<()> {
    let Err(disagreement) = check.read_point(crossings, point, index, segments) else {
        return Ok(());
    };
    let (_, owner, _) = check.point(disagreement.earlier);
    if owner == index {
        anyhow::bail!("{}", control_vertices_disagree(names[index], point.truncate()));
    }
    anyhow::bail!(
        "{}",
        controls_disagree(names[owner], names[index], point.truncate(), disagreement.earlier_z, disagreement.z)
    );
}

/// The segments of `string` meeting at vertex `at`, with how many there are:
/// one at either end, two between.
pub(crate) fn segments_at(string: &[DVec3], at: usize) -> ([[DVec3; 2]; 2], usize) {
    let before = at.checked_sub(1).map(|start| [string[start], string[at]]);
    let after = string.get(at + 1).map(|&end| [string[at], end]);
    match (before, after) {
        (Some(before), Some(after)) => ([before, after], 2),
        (Some(one), None) | (None, Some(one)) => ([one, one], 1),
        (None, None) => ([[string[at]; 2]; 2], 0),
    }
}

/// How far in plan a crossing may lie off a segment and still be on it:
/// twice the kernel's tolerance, as a crossing may join two meetings.
const ON_SEGMENT: f64 = 2.0 * kernel::XY_TOL;

/// The check every point of the controls passes on its way into the fit.
/// Each point read is checked against every point read before it within
/// [`MERGE_DISTANCE`] in plan, so the order the strings come in cannot
/// change the outcome, and it enters the fit unless one entered already
/// lies that close. Two crossings must agree within [`MERGE_HEIGHT`], any
/// other two points within [`CONTROL_AGREEMENT`], except that a point of a
/// string beside a crossing on a segment through it is that crossing as
/// well as itself: it is checked neither against the crossing nor against
/// another point beside it, and agrees with any other point at the
/// crossing's height or its own.
#[derive(Default)]
pub(crate) struct PointsCheck {
    read: Vec<ReadPoint>,
    cells: HashMap<(i64, i64), Vec<usize>>,
    near: Vec<usize>,
    /// A control's own points, crossings aside, are not checked against each
    /// other.
    between: bool,
}

/// A point the check has read.
struct ReadPoint {
    at: DVec3,
    /// The control that gave it; for a crossing, the first of its strings.
    owner: usize,
    /// For a crossing, its place among the crossings.
    crossing: Option<usize>,
    /// For a string's point, the crossings it lies beside, on a segment
    /// through them.
    beside: Vec<usize>,
    entered: bool,
}

/// A point the check refuses: the earlier point it disagrees with, by its
/// place in the reading, and the nearest of the heights each was checked
/// at, the earlier's first.
pub(crate) struct Disagreement {
    pub(crate) earlier: usize,
    pub(crate) earlier_z: f64,
    pub(crate) z: f64,
}

impl PointsCheck {
    /// A check between controls whose own points have passed it already:
    /// each point is checked against the crossings and the other controls'
    /// points only.
    pub(crate) fn between_controls() -> Self {
        Self { between: true, ..Self::default() }
    }

    /// Cells are a metre across, far wider than the distance, so every point
    /// that close lies in the cell a position falls in or one beside it.
    fn key(position: DVec2) -> (i64, i64) {
        (position.x.floor() as i64, position.y.floor() as i64)
    }

    /// Gather every point read within [`MERGE_DISTANCE`] of `position`, in
    /// the order read.
    fn gather_near(&mut self, position: DVec2) {
        self.near.clear();
        let (column, row) = Self::key(position);
        for column in column.saturating_sub(1)..=column.saturating_add(1) {
            for row in row.saturating_sub(1)..=row.saturating_add(1) {
                if let Some(cell) = self.cells.get(&(column, row)) {
                    self.near
                        .extend(cell.iter().copied().filter(|&index| self.read[index].at.truncate().distance(position) < MERGE_DISTANCE));
                }
            }
        }
        self.near.sort_unstable();
    }

    fn push(&mut self, point: ReadPoint) {
        self.cells.entry(Self::key(point.at.truncate())).or_default().push(self.read.len());
        self.read.push(point);
    }

    /// Whether a point gathered near is one the fit takes.
    fn entered_near(&self) -> bool {
        self.near.iter().any(|&index| self.read[index].entered)
    }

    /// Read crossing `index` of `crossings`, checked against the crossings
    /// read before it; every crossing is read before any string's point. The
    /// error is the earlier point it disagrees with, by its place in the
    /// reading.
    pub(crate) fn read_crossing(&mut self, crossings: &[Crossing], index: usize) -> Result<(), usize> {
        let crossing = &crossings[index];
        self.gather_near(crossing.position);
        if let Some(&earlier) = self.near.iter().find(|&&earlier| (self.read[earlier].at.z - crossing.z).abs() > MERGE_HEIGHT) {
            return Err(earlier);
        }
        let entered = !self.entered_near();
        self.push(ReadPoint {
            at: crossing.position.extend(crossing.z),
            owner: crossing.sides[0].control,
            crossing: Some(index),
            beside: Vec::new(),
            entered,
        });
        Ok(())
    }

    /// Read `point` of control `control`, which lies on `segments`: the one
    /// it densifies, or the one or two meeting at a vertex. The error is the
    /// earliest point read it disagrees with.
    pub(crate) fn read_point(&mut self, crossings: &[Crossing], point: DVec3, control: usize, segments: &[[DVec3; 2]]) -> Result<(), Disagreement> {
        self.gather_near(point.truncate());
        let on_segments = |at: DVec2| {
            segments
                .iter()
                .any(|segment| kernel::project_onto_segment(at, segment[0].truncate(), segment[1].truncate()).0.distance(at) <= ON_SEGMENT)
        };
        let beside: Vec<usize> = self
            .near
            .iter()
            .filter_map(|&earlier| self.read[earlier].crossing)
            .filter(|&index| crossings[index].sides.iter().any(|side| side.control == control) && on_segments(crossings[index].position))
            .collect();
        let one_place = |earlier: &ReadPoint| match earlier.crossing {
            Some(index) => beside.contains(&index),
            None => earlier.beside.iter().any(|index| beside.contains(index)),
        };
        // A point beside a crossing stands for it and for itself, so the two
        // are compared at the nearest of their heights.
        let nearest = |earlier: &ReadPoint| {
            heights(crossings, earlier.at.z, &earlier.beside)
                .flat_map(|other| heights(crossings, point.z, &beside).map(move |one| (other, one)))
                .min_by(|left, right| (left.0 - left.1).abs().total_cmp(&(right.0 - right.1).abs()))
                .unwrap_or((earlier.at.z, point.z))
        };
        let clash = self.near.iter().copied().find_map(|earlier| {
            let read = &self.read[earlier];
            let own = self.between && read.crossing.is_none() && read.owner == control;
            if own || one_place(read) {
                return None;
            }
            let (earlier_z, z) = nearest(read);
            (earlier_z - z)
                .abs()
                .partial_cmp(&CONTROL_AGREEMENT)
                .is_none_or(std::cmp::Ordering::is_gt)
                .then_some(Disagreement { earlier, earlier_z, z })
        });
        if let Some(disagreement) = clash {
            return Err(disagreement);
        }
        let entered = !self.entered_near();
        self.push(ReadPoint {
            at: point,
            owner: control,
            crossing: None,
            beside,
            entered,
        });
        Ok(())
    }

    /// A point read, by its place in the reading: where it is, the control
    /// that gave it, and its place among the crossings when it is one.
    pub(crate) fn point(&self, index: usize) -> (DVec3, usize, Option<usize>) {
        let read = &self.read[index];
        (read.at, read.owner, read.crossing)
    }

    /// The points the fit takes, in the order read.
    pub(crate) fn into_entered(self) -> Vec<DVec3> {
        self.read.into_iter().filter(|read| read.entered).map(|read| read.at).collect()
    }
}

/// The heights a point is checked at: its own, and those of the crossings
/// it is beside, which it stands for too.
fn heights<'a>(crossings: &'a [Crossing], own: f64, beside: &'a [usize]) -> impl Iterator<Item = f64> + 'a {
    std::iter::once(own).chain(beside.iter().map(|&index| crossings[index].z))
}

/// Two crossings too close in plan to be two points, at heights too far
/// apart to be one: the strings the later one has and the earlier lacks are
/// named against the strings the earlier has and the later lacks.
fn crossings_disagree(crossings: &[Crossing], earlier: usize, later: usize, names: &[usize]) -> String {
    let (first, second) = (&crossings[earlier], &crossings[later]);
    let lacking = |crossing: &Crossing, other: &Crossing| {
        crossing
            .sides
            .iter()
            .map(|side| side.control)
            .find(|control| other.sides.iter().all(|side| side.control != *control))
            .unwrap_or(crossing.sides[0].control)
    };
    let (one, mut other) = (lacking(first, second), lacking(second, first));
    // The same two strings crossing twice: name both of them.
    if one == other {
        other = second.sides.iter().map(|side| side.control).find(|&control| control != one).unwrap_or(other);
    }
    let mut named = [(names[one], first.z), (names[other], second.z)];
    named.sort_by_key(|&(name, _)| name);
    let [(one, low), (other, high)] = named;
    tr!(
        "cmd-reference-surface-control-strings-b-cross-near",
        a = (one + 1).to_string(),
        b = (other + 1).to_string(),
        distance = MERGE_DISTANCE.to_string(),
        x = format!("{:.3}", second.position.x),
        y = format!("{:.3}", second.position.y),
        za = format!("{low:.3}"),
        zb = format!("{high:.3}")
    )
}

/// Control points entered so far, filed by plan position so a new one finds
/// the first within [`MERGE_DISTANCE`] without walking them all.
#[derive(Default)]
pub(crate) struct PlanCells {
    points: Vec<DVec3>,
    owners: Vec<usize>,
    cells: HashMap<(i64, i64), Vec<usize>>,
}

impl PlanCells {
    /// Cells are a metre across, far wider than the distance, so a match is
    /// always in the cell a position falls in or one beside it.
    fn key(position: DVec2) -> (i64, i64) {
        (position.x.floor() as i64, position.y.floor() as i64)
    }

    pub(crate) fn add(&mut self, point: DVec3, owner: usize) {
        self.cells.entry(Self::key(point.truncate())).or_default().push(self.points.len());
        self.points.push(point);
        self.owners.push(owner);
    }

    /// The earliest point within [`MERGE_DISTANCE`] of a position, and the
    /// control that entered it.
    pub(crate) fn first_near(&self, position: DVec2) -> Option<(DVec3, usize)> {
        let (column, row) = Self::key(position);
        (column.saturating_sub(1)..=column.saturating_add(1))
            .flat_map(|column| (row.saturating_sub(1)..=row.saturating_add(1)).map(move |row| (column, row)))
            .filter_map(|key| self.cells.get(&key))
            .flatten()
            .copied()
            .filter(|&index| self.points[index].truncate().distance(position) < MERGE_DISTANCE)
            .min()
            .map(|index| (self.points[index], self.owners[index]))
    }
}

/// The controls' indices sorted by their vertices, x then y then z at each
/// in turn, a string that runs out first coming first.
pub(crate) fn canonical_order(controls: &[Vec<DVec3>]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..controls.len()).collect();
    order.sort_by(|&left, &right| {
        let (left, right) = (&controls[left], &controls[right]);
        left.iter()
            .zip(right)
            .map(|(a, b)| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)).then(a.z.total_cmp(&b.z)))
            .find(|order| order.is_ne())
            .unwrap_or_else(|| left.len().cmp(&right.len()))
    });
    order
}

/// One of the self-shape refusals, given the string's place in the selection
/// and where in plan it goes wrong.
pub(crate) type SelfShapeMessage = fn(usize, DVec2) -> String;

/// Refuse a control too short to make a segment, closed without the flag,
/// or crossing or doubling back on itself. Two controls crossing are
/// [`control_crossings`]'s concern.
///
/// The first string too short or closed without the flag is refused at once.
/// Every string crossing or doubling back is collected instead, so one
/// refusal names them all, in selection order, up to [`SELF_SHAPE_LINES`]
/// and then a count of the rest.
pub(crate) fn validate_controls(controls: &[Vec<DVec3>], names: &[usize], cancelled: &dyn Fn() -> bool) -> Result<()> {
    let mut misshapen: Vec<(usize, DVec3, SelfShapeMessage)> = Vec::new();
    for (index, control) in controls.iter().enumerate() {
        stop_if_cancelled(cancelled)?;
        if control.len() < MINIMUM_CONTROL_VERTICES {
            anyhow::bail!("{}", too_few_control_vertices(names[index], control.len()));
        }
        let plan: Vec<DVec2> = control.iter().map(|vertex| vertex.truncate()).collect();
        if plan.len() >= 4 && plan[0].distance(plan[plan.len() - 1]) <= kernel::XY_TOL {
            anyhow::bail!("{}", control_ends_where_it_starts(names[index]));
        }
        if let Some(turn) = turning_back(&plan) {
            misshapen.push((names[index], control[turn], control_doubles_back));
        } else if let Some(contact) = self_intersection(&plan, false) {
            let z = elevation_along(control[contact.segment], control[contact.segment + 1], contact.along);
            misshapen.push((names[index], contact.point.extend(z), control_self_crossing));
        }
    }
    if misshapen.is_empty() {
        return Ok(());
    }
    misshapen.sort_by_key(|&(name, ..)| name);
    let mut report = misshapen
        .iter()
        .take(SELF_SHAPE_LINES)
        .map(|&(name, position, message)| message(name, position.truncate()))
        .collect::<Vec<_>>()
        .join("\n");
    if misshapen.len() > SELF_SHAPE_LINES {
        report.push_str(&tr!("cmd-reference-surface-and-more", more = (misshapen.len() - SELF_SHAPE_LINES).to_string()));
    }
    Err(MisshapenControls {
        refused: misshapen.iter().map(|&(name, ..)| name).collect(),
        positions: misshapen.iter().map(|&(_, position, _)| position).collect(),
        report,
    }
    .into())
}

/// The refusal for controls crossing or doubling back on themselves, or
/// read before the build as gone, empty or not numbers, typed so the UI
/// thread can select the strings it names without reading the text:
/// `refused` holds every one, past the report's line limit too, by place in
/// the selection counting from zero, in that order. `positions` holds where
/// each goes wrong, in the same order: the turn vertex, the self-crossing
/// point at the height of the first segment there, or a vertex of a string
/// that is not numbers; empty when the one string refused has none.
#[derive(Debug)]
pub(crate) struct MisshapenControls {
    pub(crate) refused: Vec<usize>,
    pub(crate) positions: Vec<DVec3>,
    pub(crate) report: String,
}

impl std::fmt::Display for MisshapenControls {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.report)
    }
}

impl std::error::Error for MisshapenControls {}

/// Where an open string first turns back along the segment before it, its
/// far end on that segment's line and pointing the other way: the vertex
/// the string turns at, by its place along the string.
pub(crate) fn turning_back(string: &[DVec2]) -> Option<usize> {
    string
        .windows(3)
        .position(|corner| {
            let (before, after) = (corner[1] - corner[0], corner[2] - corner[1]);
            before.perp_dot(corner[2] - corner[0]).abs() <= kernel::XY_TOL * before.length() && before.dot(after) < 0.0
        })
        .map(|start| start + 1)
}

/// A plan position more than one control runs through: one vertex of the
/// surface, at the mean of the elevations its controls give it, one each,
/// so it does not hang on which control comes first or where its vertices
/// sit, with every control that reaches it constrained through it.
pub(crate) struct Crossing {
    pub(crate) position: DVec2,
    pub(crate) z: f64,
    pub(crate) sides: Vec<CrossingSide>,
}

impl Crossing {
    /// Keep one part per control segment. Three strings through one point meet
    /// pairwise, so each of their parts arrives twice. Whether its control is
    /// new, as a vertex on the crossing brings two segments of one control.
    fn add(&mut self, side: CrossingSide) -> bool {
        let new = !self.sides.iter().any(|kept| kept.control == side.control);
        if !self.sides.iter().any(|kept| (kept.control, kept.segment) == (side.control, side.segment)) {
            self.sides.push(side);
        }
        new
    }
}

/// How one control reaches a crossing: which of its segments.
pub(crate) struct CrossingSide {
    pub(crate) control: usize,
    pub(crate) segment: usize,
}

/// Every plan position two different controls run through, each with the
/// elevation both give it. Read off the strings themselves, before anything is
/// inserted, so controls that disagree leave no half-built surface behind.
pub(crate) fn control_crossings(controls: &[Vec<DVec3>], names: &[usize], cancelled: &dyn Fn() -> bool) -> Result<Vec<Crossing>> {
    let mut crossings: Vec<Crossing> = Vec::new();
    // Every segment of every control on one grid, so only segments that come
    // near each other are compared; the pairs are then taken in the order a
    // walk over every pair would meet them, so the first refusal is the same.
    let segments: Vec<(usize, usize)> = controls
        .iter()
        .enumerate()
        .flat_map(|(index, control)| (0..control.len() - 1).map(move |segment| (index, segment)))
        .collect();
    let grid = BoxGrid::new(
        segments
            .iter()
            .map(|&(index, segment)| segment_box(controls[index][segment].truncate(), controls[index][segment + 1].truncate()))
            .collect(),
    );
    let mut found = CrossingCells::default();
    // The sum and count of the heights of each crossing's controls.
    let mut heights: Vec<(f64, usize)> = Vec::new();
    let (mut near, mut pairs) = (Vec::new(), Vec::new());
    for (left, first) in controls.iter().enumerate() {
        stop_if_cancelled(cancelled)?;
        pairs.clear();
        for a in 0..first.len() - 1 {
            let (low, high) = segment_box(first[a].truncate(), first[a + 1].truncate());
            grid.overlapping(low, high, &mut near);
            pairs.extend(near.iter().map(|&other| segments[other]).filter(|&(right, _)| right > left).map(|(right, b)| (right, a, b)));
        }
        pairs.sort_unstable();
        for &(right, a, b) in &pairs {
            let second = &controls[right];
            let meeting = kernel::segment_segment(first[a].truncate(), first[a + 1].truncate(), second[b].truncate(), second[b + 1].truncate());
            // A string ending on another reads the same as one running
            // across it: one plan position, two interpretations of it.
            let (point, t, u) = match meeting {
                SegSeg::Disjoint => continue,
                SegSeg::CollinearOverlap { .. } => anyhow::bail!("{}", controls_along_each_other(names[left], names[right])),
                SegSeg::Crossing { point, t, u } | SegSeg::Touching { point, t, u } => (point, t, u),
            };
            let (one, one_z) = crossing_side(first, left, a, t, point);
            let (other, other_z) = crossing_side(second, right, b, u, point);
            if (one_z - other_z).abs() > CONTROL_AGREEMENT {
                anyhow::bail!("{}", controls_disagree(names[left], names[right], point, one_z, other_z));
            }
            match found.first_near(&crossings, point) {
                Some(index) => {
                    for (side, z) in [(one, one_z), (other, other_z)] {
                        if crossings[index].add(side) {
                            heights[index].0 += z;
                            heights[index].1 += 1;
                        }
                    }
                }
                None => {
                    found.add(point, crossings.len());
                    crossings.push(Crossing {
                        position: point,
                        z: one_z,
                        sides: vec![one, other],
                    });
                    heights.push((one_z + other_z, 2));
                }
            }
        }
    }
    for (crossing, (sum, count)) in crossings.iter_mut().zip(heights) {
        crossing.z = sum / count as f64;
    }
    Ok(crossings)
}

/// The crossings found so far, filed by plan position so a new meeting finds
/// the first one within the kernel's tolerance without walking them all.
#[derive(Default)]
pub(crate) struct CrossingCells {
    cells: HashMap<(i64, i64), Vec<usize>>,
}

impl CrossingCells {
    /// Cells are a metre across, far wider than the tolerance, so a match is
    /// always in the cell a position falls in or one beside it.
    fn key(position: DVec2) -> (i64, i64) {
        (position.x.floor() as i64, position.y.floor() as i64)
    }

    fn add(&mut self, position: DVec2, index: usize) {
        self.cells.entry(Self::key(position)).or_default().push(index);
    }

    /// The earliest crossing within [`kernel::XY_TOL`] of a position.
    fn first_near(&self, crossings: &[Crossing], position: DVec2) -> Option<usize> {
        let (column, row) = Self::key(position);
        (column.saturating_sub(1)..=column.saturating_add(1))
            .flat_map(|column| (row.saturating_sub(1)..=row.saturating_add(1)).map(move |row| (column, row)))
            .filter_map(|key| self.cells.get(&key))
            .flatten()
            .copied()
            .filter(|&index| crossings[index].position.distance(position) <= kernel::XY_TOL)
            .min()
    }
}

/// One control's part in a crossing, with the elevation it reads there: its
/// own vertex's when the crossing lands on one, the segment's otherwise.
pub(crate) fn crossing_side(control: &[DVec3], index: usize, segment: usize, along: f64, point: DVec2) -> (CrossingSide, f64) {
    let vertex = nearer_end(point, control[segment].truncate(), control[segment + 1].truncate()).map(|end| segment + end);
    let z = match vertex {
        Some(vertex) => control[vertex].z,
        None => elevation_along(control[segment], control[segment + 1], along),
    };
    (CrossingSide { control: index, segment }, z)
}

/// Whether any two of a string's segments meet away from the ends they share
/// with their neighbours, the first and last included when it is closed.
/// Any contact counts: an open string touching itself gives one plan
/// position two elevations, and a ring doing so bounds no single area.
pub(crate) fn self_intersects(points: &[DVec2], closed: bool) -> bool {
    self_intersection(points, closed).is_some()
}

/// Where a string meets itself: the plan point, and the segment of the pair
/// met first along the string with how far along it the point lies.
pub(crate) struct SelfContact {
    pub(crate) point: DVec2,
    pub(crate) segment: usize,
    pub(crate) along: f64,
}

/// Where a string first meets itself as [`self_intersects`] judges it,
/// segments taken in order along the string: the crossing or touching point,
/// or where the overlap starts when two segments run along each other.
pub(crate) fn self_intersection(points: &[DVec2], closed: bool) -> Option<SelfContact> {
    // A vertex within the plan tolerance of the next is a stutter in the
    // digitising, not a segment: step over it, keeping the later vertex so
    // each segment kept is the string's own.
    let kept: Vec<usize> = (0..points.len())
        .filter(|&vertex| points.get(vertex + 1).is_none_or(|next| points[vertex].distance(*next) > kernel::XY_TOL))
        .collect();
    let points: Vec<DVec2> = kept.iter().map(|&vertex| points[vertex]).collect();
    let count = points.len();
    let segments = if closed { count } else { count.saturating_sub(1) };
    let grid = BoxGrid::new((0..segments).map(|segment| segment_box(points[segment], points[(segment + 1) % count])).collect());
    let mut near = Vec::new();
    (0..segments).find_map(|first| {
        let (low, high) = segment_box(points[first], points[(first + 1) % count]);
        grid.overlapping(low, high, &mut near);
        near.iter()
            .filter(|&&second| second >= first + 2 && !(closed && first == 0 && second == segments - 1))
            .find_map(
                |&second| match kernel::segment_segment(points[first], points[(first + 1) % count], points[second], points[(second + 1) % count]) {
                    SegSeg::Disjoint => None,
                    SegSeg::Crossing { point, t, .. } | SegSeg::Touching { point, t, .. } => Some(SelfContact {
                        point,
                        segment: kept[first],
                        along: t,
                    }),
                    SegSeg::CollinearOverlap { t0, .. } => Some(SelfContact {
                        point: points[first].lerp(points[(first + 1) % count], t0),
                        segment: kept[first],
                        along: t0,
                    }),
                },
            )
    })
}

/// How far a box is widened for the grids below: twice the kernel's plan
/// tolerance, so rounding can never hide a pair the kernel would call close.
pub(crate) const SEARCH_MARGIN: f64 = 2.0 * kernel::XY_TOL;

/// A segment's plan box, widened by [`SEARCH_MARGIN`].
pub(crate) fn segment_box(start: DVec2, end: DVec2) -> (DVec2, DVec2) {
    (start.min(end) - DVec2::splat(SEARCH_MARGIN), start.max(end) + DVec2::splat(SEARCH_MARGIN))
}

/// The most cells one box is filed in; a wider box is checked against every
/// query instead.
const MOST_CELLS_PER_BOX: u64 = 64;

/// Boxes filed on a grid of cells, each about the size of a typical box,
/// the cells hashed into as many buckets as there are boxes, so only
/// boxes sharing a bucket are compared and far-off boxes cost no empty cells.
pub(crate) struct BoxGrid {
    low: DVec2,
    /// A cell's width and height.
    cell: DVec2,
    /// Where each bucket's run of `members` starts, one more than the buckets.
    starts: Vec<usize>,
    members: Vec<usize>,
    /// The boxes too wide to file, found by every query.
    wide: Vec<usize>,
    boxes: Vec<(DVec2, DVec2)>,
}

/// The size nine in ten boxes come within, measured by `size`, so they
/// span a few cells at most; infinite when no box has a finite size.
fn typical_size(boxes: &[(DVec2, DVec2)], size: fn(DVec2) -> f64) -> f64 {
    let mut sizes: Vec<f64> = boxes.iter().map(|(from, to)| size(*to - *from)).collect();
    sizes.sort_unstable_by(f64::total_cmp);
    sizes
        .get(sizes.len() * 9 / 10)
        .copied()
        .filter(|cell| cell.is_finite() && *cell > 0.0)
        .unwrap_or(f64::INFINITY)
}

impl BoxGrid {
    pub(crate) fn new(boxes: Vec<(DVec2, DVec2)>) -> Self {
        let cell = typical_size(&boxes, DVec2::max_element);
        Self::filed(boxes, DVec2::splat(cell))
    }

    /// As [`BoxGrid::new`], with a cell's width and height taken from the
    /// boxes' widths and heights apart, so long thin boxes side by side (the
    /// edges of a dense zigzag) each fall in few cells and share them with
    /// few others.
    pub(crate) fn fitted(boxes: Vec<(DVec2, DVec2)>) -> Self {
        let cell = DVec2::new(typical_size(&boxes, |size| size.x), typical_size(&boxes, |size| size.y));
        Self::filed(boxes, cell)
    }

    fn filed(boxes: Vec<(DVec2, DVec2)>, cell: DVec2) -> Self {
        let low = boxes.iter().fold(DVec2::INFINITY, |low, (from, _)| low.min(*from));
        let low = if boxes.is_empty() { DVec2::ZERO } else { low };
        let buckets = boxes.len().max(1);
        let mut grid = Self {
            low,
            cell,
            starts: vec![0; buckets + 1],
            members: Vec::new(),
            wide: Vec::new(),
            boxes,
        };
        let mut filed: Vec<Vec<usize>> = Vec::new();
        for index in 0..grid.boxes.len() {
            match grid.cells(grid.boxes[index]) {
                Some(cells) => filed.push(cells.collect()),
                None => {
                    grid.wide.push(index);
                    filed.push(Vec::new());
                }
            }
        }
        for cells in &filed {
            for &bucket in cells {
                grid.starts[bucket + 1] += 1;
            }
        }
        for bucket in 0..buckets {
            grid.starts[bucket + 1] += grid.starts[bucket];
        }
        let mut next = grid.starts.clone();
        let mut members = vec![0; grid.starts[buckets]];
        for (index, cells) in filed.iter().enumerate() {
            for &bucket in cells {
                members[next[bucket]] = index;
                next[bucket] += 1;
            }
        }
        grid.members = members;
        grid
    }

    /// The buckets of the cells a box covers, or none when it covers more
    /// than [`MOST_CELLS_PER_BOX`].
    fn cells(&self, (from, to): (DVec2, DVec2)) -> Option<impl Iterator<Item = usize> + use<>> {
        let (first, last) = (self.cell_at(from), self.cell_at(to));
        let across = |axis: fn((i64, i64)) -> i64| (axis(last) as i128 - axis(first) as i128 + 1) as u64;
        if across(|cell| cell.0).saturating_mul(across(|cell| cell.1)) > MOST_CELLS_PER_BOX {
            return None;
        }
        let buckets = self.starts.len() - 1;
        Some((first.1..=last.1).flat_map(move |row| (first.0..=last.0).map(move |column| bucket_of(column, row, buckets))))
    }

    /// The cell a position falls in.
    fn cell_at(&self, position: DVec2) -> (i64, i64) {
        let cell = ((position - self.low) / self.cell).floor();
        (cell.x as i64, cell.y as i64)
    }

    /// The boxes overlapping the one from `from` to `to`, ascending and each
    /// once, into `found`.
    pub(crate) fn overlapping(&self, from: DVec2, to: DVec2, found: &mut Vec<usize>) {
        found.clear();
        let overlaps = |index: usize| {
            let (low, high) = self.boxes[index];
            low.x <= to.x && from.x <= high.x && low.y <= to.y && from.y <= high.y
        };
        match self.cells((from, to)) {
            Some(cells) => {
                for bucket in cells {
                    found.extend(self.members[self.starts[bucket]..self.starts[bucket + 1]].iter().copied().filter(|&index| overlaps(index)));
                }
                found.extend(self.wide.iter().copied().filter(|&index| overlaps(index)));
            }
            None => found.extend((0..self.boxes.len()).filter(|&index| overlaps(index))),
        }
        found.sort_unstable();
        found.dedup();
    }
}

/// Which of `buckets` a cell is filed in.
fn bucket_of(column: i64, row: i64, buckets: usize) -> usize {
    let hash = (column as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (row as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    ((hash ^ (hash >> 32)) % buckets as u64) as usize
}

/// A cell size giving a grid about as many cells as it holds items, never so
/// fine along a thin span that one axis outnumbers them. A span that is not
/// a finite size gets one cell.
pub(crate) fn grid_cell(span: DVec2, count: usize) -> f64 {
    let count = count.max(1) as f64;
    let cell = (span.x * span.y / count).sqrt().max(span.x.max(span.y) / count);
    if cell.is_finite() && cell > 0.0 { cell } else { f64::INFINITY }
}

pub(crate) fn cells_across(span: f64, cell: f64) -> usize {
    ((span / cell).floor() as usize).saturating_add(1)
}

/// The cell a coordinate falls in along one axis, clamped to the grid.
pub(crate) fn cell_of(value: f64, low: f64, cell: f64, count: usize) -> usize {
    (((value - low) / cell).floor().max(0.0) as usize).min(count - 1)
}

/// Which end of a segment a point coincides with in plan, when it coincides
/// with either: `0` for the start, `1` for the end.
pub(crate) fn nearer_end(point: DVec2, start: DVec2, end: DVec2) -> Option<usize> {
    let (to_start, to_end) = (point.distance(start), point.distance(end));
    (to_start.min(to_end) <= kernel::XY_TOL).then(|| usize::from(to_end < to_start))
}

/// The elevation a segment has at the fraction `along` of its length.
pub(crate) fn elevation_along(start: DVec3, end: DVec3, along: f64) -> f64 {
    start.z + (end.z - start.z) * along
}
