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
        rbf::{DEFAULT_SPACING, MERGE_DISTANCE},
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
    if cancelled() {
        anyhow::bail!("{}", tr!("common-cancelled"));
    }
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
/// same point and goes in once; the two must agree on its height.
pub(crate) fn control_points(controls: &[Vec<DVec3>], names: &[usize], crossings: &[Crossing], cancelled: &dyn Fn() -> bool) -> Result<Vec<DVec3>> {
    control_points_spaced(controls, names, crossings, DEFAULT_SPACING, cancelled)
}

/// [`control_points`] with the segments densified at `spacing` in plan;
/// an infinite spacing enters the crossings and the vertices alone.
pub(crate) fn control_points_spaced(controls: &[Vec<DVec3>], names: &[usize], crossings: &[Crossing], spacing: f64, cancelled: &dyn Fn() -> bool) -> Result<Vec<DVec3>> {
    let mut entered = PlanCells::default();
    for crossing in crossings {
        entered.add(crossing.position.extend(crossing.z), crossing.sides[0].control);
    }
    for (index, control) in controls.iter().enumerate() {
        stop_if_cancelled(cancelled)?;
        let mut enter = |point: DVec3| -> Result<()> {
            match entered.first_near(point.truncate()) {
                None => entered.add(point, index),
                Some((kept, owner)) if (kept.z - point.z).abs() > CONTROL_AGREEMENT => {
                    if owner == index {
                        anyhow::bail!("{}", control_vertices_disagree(names[index], point.truncate()));
                    }
                    anyhow::bail!("{}", controls_disagree(names[owner], names[index], point.truncate(), kept.z, point.z));
                }
                Some(_) => {}
            }
            Ok(())
        };
        enter(control[0])?;
        for segment in control.windows(2) {
            stop_if_cancelled(cancelled)?;
            let (start, end) = (segment[0], segment[1]);
            let pieces = (start.truncate().distance(end.truncate()) / spacing).ceil().max(1.0) as usize;
            for step in 1..pieces {
                // A stray segment kilometres long enters millions of points.
                if step % CANCEL_POLL_STEPS == 0 {
                    stop_if_cancelled(cancelled)?;
                }
                enter(start + (end - start) * step as f64 / pieces as f64)?;
            }
            enter(end)?;
        }
    }
    Ok(entered.points)
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
/// surface, at the elevation they agree on, with every control that reaches it
/// constrained through it.
pub(crate) struct Crossing {
    pub(crate) position: DVec2,
    pub(crate) z: f64,
    pub(crate) sides: Vec<CrossingSide>,
}

impl Crossing {
    /// Keep one part per control segment. Three strings through one point meet
    /// pairwise, so each of their parts arrives twice.
    fn add(&mut self, side: CrossingSide) {
        if !self.sides.iter().any(|kept| (kept.control, kept.segment) == (side.control, side.segment)) {
            self.sides.push(side);
        }
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
                    crossings[index].add(one);
                    crossings[index].add(other);
                }
                None => {
                    found.add(point, crossings.len());
                    crossings.push(Crossing {
                        position: point,
                        z: one_z,
                        sides: vec![one, other],
                    });
                }
            }
        }
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
                |&second| match kernel::segment_segment(points[first], points[first + 1], points[second], points[(second + 1) % count]) {
                    SegSeg::Disjoint => None,
                    SegSeg::Crossing { point, t, .. } | SegSeg::Touching { point, t, .. } => Some(SelfContact { point, segment: first, along: t }),
                    SegSeg::CollinearOverlap { t0, .. } => Some(SelfContact {
                        point: points[first].lerp(points[first + 1], t0),
                        segment: first,
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

/// Boxes filed on a uniform grid, so only boxes sharing a cell are compared.
pub(crate) struct BoxGrid {
    low: DVec2,
    cell: f64,
    columns: usize,
    rows: usize,
    /// Where each cell's run of `members` starts, one more than the cells.
    starts: Vec<usize>,
    members: Vec<usize>,
    boxes: Vec<(DVec2, DVec2)>,
}

impl BoxGrid {
    pub(crate) fn new(boxes: Vec<(DVec2, DVec2)>) -> Self {
        let (low, high) = boxes
            .iter()
            .fold((DVec2::INFINITY, DVec2::NEG_INFINITY), |(low, high), (from, to)| (low.min(*from), high.max(*to)));
        let (low, span) = if boxes.is_empty() { (DVec2::ZERO, DVec2::ZERO) } else { (low, high - low) };
        let cell = grid_cell(span, boxes.len());
        let (columns, rows) = (cells_across(span.x, cell), cells_across(span.y, cell));
        let mut grid = Self {
            low,
            cell,
            columns,
            rows,
            starts: vec![0; columns * rows + 1],
            members: Vec::new(),
            boxes,
        };
        for index in 0..grid.boxes.len() {
            for cell in grid.cells(grid.boxes[index]) {
                grid.starts[cell + 1] += 1;
            }
        }
        for cell in 0..columns * rows {
            grid.starts[cell + 1] += grid.starts[cell];
        }
        let mut next = grid.starts.clone();
        let mut members = vec![0; grid.starts[columns * rows]];
        for index in 0..grid.boxes.len() {
            for cell in grid.cells(grid.boxes[index]) {
                members[next[cell]] = index;
                next[cell] += 1;
            }
        }
        grid.members = members;
        grid
    }

    /// The cells a box covers, clamped to the grid.
    fn cells(&self, (from, to): (DVec2, DVec2)) -> impl Iterator<Item = usize> + use<> {
        let columns = self.columns;
        let (first_column, last_column) = (cell_of(from.x, self.low.x, self.cell, columns), cell_of(to.x, self.low.x, self.cell, columns));
        let (first_row, last_row) = (cell_of(from.y, self.low.y, self.cell, self.rows), cell_of(to.y, self.low.y, self.cell, self.rows));
        (first_row..=last_row).flat_map(move |row| (first_column..=last_column).map(move |column| row * columns + column))
    }

    /// The boxes overlapping the one from `from` to `to`, ascending and each
    /// once, into `found`.
    pub(crate) fn overlapping(&self, from: DVec2, to: DVec2, found: &mut Vec<usize>) {
        found.clear();
        for cell in self.cells((from, to)) {
            for &index in &self.members[self.starts[cell]..self.starts[cell + 1]] {
                let (low, high) = self.boxes[index];
                if low.x <= to.x && from.x <= high.x && low.y <= to.y && from.y <= high.y {
                    found.push(index);
                }
            }
        }
        found.sort_unstable();
        found.dedup();
    }
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
