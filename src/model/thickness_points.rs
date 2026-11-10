//! Thickness points: the true thickness of one working section at each hole
//! that holds it, and at each measured roof and floor pair, measured square
//! to the bedding of a reference surface.
//!
//! True thickness is the roof-to-floor vector taken on the bedding normal,
//! one formula for every hole whatever its angle to the seam [Dennison 1968;
//! Heckscher & Saunders, "Precision in the dip"]. The bedding is the
//! reference surface's slope at the roof, read off its own grid nodes, so a
//! surface from a saved project measures as well as one built this session.
//!
//! Beside them, the misfit of a prismoid grid at the same picks and at
//! quality points: always the model less the pick.

use glam::{DVec2, DVec3};

use crate::{
    app::jobs::CancelFlag,
    i18n::tr,
    model::{
        Command,
        drill_hole::{DrillHole, DrillHoleDataset, DrillHoleId, DrillValue, section_run},
        grid_surface::GridSurface,
        prismoid_grid::{PlanMiss, PrismoidGrid},
        rbf::Dip,
        triangulation::{QualityMark, same_quality_name},
    },
};

/// Where a roof and floor came from.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum InterceptSource {
    /// A hole, by id, with the down-hole depths of the roof and floor.
    Hole { dhid: String, roof_depth: f64, floor_depth: Option<f64> },
    /// A measured pair, by its id and its line in the file (the header is
    /// line 1).
    Measured { id: String, line: usize },
}

/// One roof and floor of the working section, before it is measured.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Intercept {
    pub(crate) source: InterceptSource,
    pub(crate) roof: DVec3,
    /// `None` where the hole ends inside the section.
    pub(crate) floor: Option<DVec3>,
    /// The section recurs lower in the hole (a possible fault repeat): the
    /// uppermost run was taken.
    pub(crate) repeated: bool,
}

/// Why a hole or a measured pair gave no point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LeftOutReason {
    /// The hole ends inside the section, so it has a roof and no floor.
    NoFloor,
    /// The floor lies above the roof: overturned, out of scope.
    Overturned,
    /// The roof's plan position is off the reference surface.
    OutsideSurface,
    /// A measured pair with a blank or non-numeric coordinate.
    MissingValue,
    /// The hole has no trace to place the section on.
    NoTrace,
}

/// A hole or measured pair that gave no point, and why.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LeftOut {
    pub(crate) source: InterceptSource,
    pub(crate) reason: LeftOutReason,
}

/// One measured thickness, placed at its roof.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ThicknessPoint {
    pub(crate) source: InterceptSource,
    pub(crate) roof: DVec3,
    pub(crate) floor: DVec3,
    /// Square to the bedding.
    pub(crate) true_thickness: f64,
    /// The vertical thickness that true thickness makes under this dip.
    pub(crate) vertical_thickness: f64,
    /// The straight distance from roof to floor.
    pub(crate) along_hole: f64,
    /// The reference surface's dip and dip direction at the roof.
    pub(crate) dip: Dip,
    pub(crate) repeated: bool,
}

/// What one run makes: the points, then everything left out, holes first,
/// then measured pairs in file order.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ThicknessRun {
    pub(crate) points: Vec<ThicknessPoint>,
    pub(crate) left_out: Vec<LeftOut>,
}

/// Why a measured pairs file could not be read at all.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PairsError {
    /// Not text, or not CSV.
    Unreadable(String),
    /// Required columns absent from the header, by name.
    MissingColumns(Vec<String>),
}

/// The columns a measured pairs file must carry, in any order.
pub(crate) const PAIR_COLUMNS: [&str; 7] = ["id", "roof_x", "roof_y", "roof_z", "floor_x", "floor_y", "floor_z"];

/// True and vertical thickness for a roof and floor under a surface of
/// slope `gradient` (dz/dx, dz/dy).
pub(crate) fn true_thickness(roof: DVec3, floor: DVec3, gradient: DVec2) -> (f64, f64) {
    let stretch = (1.0 + gradient.x * gradient.x + gradient.y * gradient.y).sqrt();
    let normal = DVec3::new(-gradient.x, -gradient.y, 1.0) / stretch;
    let thickness = (floor - roof).dot(normal).abs();
    (thickness, thickness * stretch)
}

/// The roof and floor of the section `codes` names in `field`, on the
/// hole's trace; `None` where the hole never holds it.
pub(crate) fn hole_intercept(hole: &DrillHole, field: &str, codes: &[String]) -> Option<Result<Intercept, LeftOut>> {
    let run = section_run(hole, field, codes)?;
    let hole_source = |floor_depth| InterceptSource::Hole {
        dhid: hole.dhid.clone(),
        roof_depth: run.top,
        floor_depth,
    };
    let Some(roof) = hole.position_at_depth(run.top) else {
        return Some(Err(LeftOut {
            source: hole_source(Some(run.base)),
            reason: LeftOutReason::NoTrace,
        }));
    };
    // A floor past the last station would be the toe again, not a floor.
    let traced = hole.trace.last().is_some_and(|toe| run.base <= toe.depth + 1e-9);
    let floor = if run.closed && traced { hole.position_at_depth(run.base) } else { None };
    let source = hole_source(floor.map(|_| run.base));
    Some(Ok(Intercept {
        source,
        roof,
        floor,
        repeated: run.runs > 1,
    }))
}

/// Every row of a measured pairs file: the pairs read, and the rows left
/// out for a blank or non-numeric coordinate.
pub(crate) fn read_pairs(bytes: &[u8]) -> Result<(Vec<Intercept>, Vec<LeftOut>), PairsError> {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let mut reader = csv::ReaderBuilder::new().has_headers(true).flexible(true).trim(csv::Trim::All).from_reader(bytes);
    let unreadable = |error: csv::Error| PairsError::Unreadable(error.to_string());
    let header = reader.headers().map_err(unreadable)?.clone();
    let columns: Vec<Option<usize>> = PAIR_COLUMNS
        .iter()
        .map(|name| header.iter().position(|cell| cell.trim().eq_ignore_ascii_case(name)))
        .collect();
    let missing: Vec<String> = PAIR_COLUMNS.iter().zip(&columns).filter(|(_, at)| at.is_none()).map(|(name, _)| name.to_string()).collect();
    if !missing.is_empty() {
        return Err(PairsError::MissingColumns(missing));
    }
    let columns: Vec<usize> = columns.into_iter().flatten().collect();

    let mut pairs = Vec::new();
    let mut left_out = Vec::new();
    for record in reader.records() {
        let record = record.map_err(unreadable)?;
        if record.iter().all(str::is_empty) {
            continue;
        }
        let line = record.position().map_or(0, |position| position.line() as usize);
        let id = record.get(columns[0]).unwrap_or_default().to_string();
        let number = |column: usize| record.get(column).and_then(|cell| cell.parse::<f64>().ok()).filter(|value| value.is_finite());
        let values: Option<Vec<f64>> = columns[1..].iter().map(|&column| number(column)).collect();
        let source = InterceptSource::Measured { id, line };
        match values.as_deref() {
            Some(&[rx, ry, rz, fx, fy, fz]) => pairs.push(Intercept {
                source,
                roof: DVec3::new(rx, ry, rz),
                floor: Some(DVec3::new(fx, fy, fz)),
                repeated: false,
            }),
            _ => left_out.push(LeftOut {
                source,
                reason: LeftOutReason::MissingValue,
            }),
        }
    }
    Ok((pairs, left_out))
}

/// The interval column a hole's true thickness is saved in. Fixed and never
/// translated, so a saved file reads the same in any language.
pub(crate) const TRUE_THICKNESS_COLUMN: &str = "True thickness";

/// What saving one run to a dataset's column does, for the report.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ColumnSummary {
    /// Roof intervals given a thickness.
    pub(crate) written: usize,
    /// Cells that held an earlier run's value, overwritten or cleared.
    pub(crate) replaced: usize,
    /// Holes left out whose earlier value was cleared.
    pub(crate) cleared: usize,
}

/// The dataset already holds a column of [`TRUE_THICKNESS_COLUMN`]'s name
/// that no Incline tool wrote: logged data, never written over.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ColumnClash;

/// The edit saving one run's thicknesses to `dataset`, or `None` when it
/// changes nothing. `holes` names each hole holding the section by index,
/// with its true thickness, or `None` where the run left it out. The value
/// goes on the roof interval of the section's uppermost run only; every
/// other interval of that run, partings included, is left blank.
pub(crate) fn thickness_column_edit(
    id: DrillHoleId,
    dataset: &DrillHoleDataset,
    field: &str,
    codes: &[String],
    holes: &[(usize, Option<f64>)],
) -> Result<Option<(Command, ColumnSummary)>, ColumnClash> {
    let column = TRUE_THICKNESS_COLUMN;
    let derived = dataset.derived_columns.contains(column);
    if !derived && dataset.fields.iter().any(|field| field.key == column) {
        return Err(ColumnClash);
    }
    let mut before = Vec::new();
    let mut after = Vec::new();
    let mut summary = ColumnSummary::default();
    for &(index, thickness) in holes {
        let Some(hole) = dataset.holes.get(index) else {
            continue;
        };
        let Some(run) = section_run(hole, field, codes) else {
            continue;
        };
        let mut inside: Vec<usize> = (0..hole.intervals.len())
            .filter(|&slot| {
                let interval = &hole.intervals[slot];
                interval.from >= run.top && interval.to <= run.base
            })
            .collect();
        inside.sort_by(|a, b| hole.intervals[*a].from.total_cmp(&hole.intervals[*b].from));
        let roof = inside
            .iter()
            .copied()
            .find(|&slot| matches!(hole.intervals[slot].values.get(field), Some(DrillValue::Category(code)) if codes.contains(code)));
        let mut had = false;
        for slot in inside {
            let old = hole.intervals[slot].values.get(column).cloned();
            let new = if Some(slot) == roof { thickness.map(DrillValue::Numeric) } else { None };
            had |= old.is_some();
            if old != new {
                summary.replaced += usize::from(old.is_some());
                before.push((index, slot, old));
                after.push((index, slot, new));
            }
        }
        match thickness {
            Some(_) => summary.written += usize::from(roof.is_some()),
            None => summary.cleared += usize::from(had),
        }
    }
    if after.is_empty() {
        return Ok(None);
    }
    let command = Command::WriteIntervalColumn {
        dataset: id,
        column: column.to_owned(),
        before,
        after,
        derived: (derived, true),
    };
    Ok(Some((command, summary)))
}

/// Measure every intercept against `reference`. Fails only when cancelled.
pub(crate) fn thickness_points(intercepts: Vec<Intercept>, reference: &GridSurface, cancel: &CancelFlag) -> anyhow::Result<ThicknessRun> {
    let mut run = ThicknessRun::default();
    for (index, intercept) in intercepts.into_iter().enumerate() {
        if index % 256 == 0 && cancel.is_cancelled() {
            anyhow::bail!("{}", tr!("common-cancelled"));
        }
        let Intercept { source, roof, floor, repeated } = intercept;
        let reason = match floor {
            None => Some(LeftOutReason::NoFloor),
            Some(floor) if floor.z > roof.z => Some(LeftOutReason::Overturned),
            Some(_) if !reference.covers(roof.truncate()) => Some(LeftOutReason::OutsideSurface),
            Some(_) => None,
        };
        let (Some(floor), None) = (floor, reason) else {
            run.left_out.push(LeftOut {
                source,
                reason: reason.unwrap_or(LeftOutReason::NoFloor),
            });
            continue;
        };
        let at = roof.truncate();
        let gradient = reference.slope(at);
        let dip = reference.dip(at);
        let (true_thickness, vertical_thickness) = true_thickness(roof, floor, gradient);
        run.points.push(ThicknessPoint {
            source,
            roof,
            floor,
            true_thickness,
            vertical_thickness,
            along_hole: roof.distance(floor),
            dip,
            repeated,
        });
    }
    Ok(run)
}

/// What one misfit row measures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MisfitMeasure {
    /// Roof RL at the roof pick.
    Roof,
    /// Floor RL at the floor pick.
    Floor,
    /// Vertical thickness at the roof pick.
    Thickness,
    /// True thickness, square to the model's roof at the roof pick.
    TrueThickness,
    /// The points of the quality layer at this index of those measured.
    Quality(usize),
}

impl MisfitMeasure {
    /// The measures every hole gives, in row order.
    pub(crate) const HOLE: [Self; 4] = [Self::Roof, Self::Floor, Self::Thickness, Self::TrueThickness];

    /// Where the measure's totals sit: the hole measures, then each quality.
    pub(crate) fn slot(self) -> usize {
        match self {
            Self::Roof => 0,
            Self::Floor => 1,
            Self::Thickness => 2,
            Self::TrueThickness => 3,
            Self::Quality(index) => Self::HOLE.len() + index,
        }
    }
}

/// Why a misfit row was not compared.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum NotCompared {
    /// The hole never holds the seam.
    NoPick,
    NoTrace,
    /// The seam recurs lower in the hole.
    Repeated,
    /// The hole ends inside the seam.
    NoFloor,
    /// The floor lies above the roof.
    Overturned,
    /// The hole runs through the seam flatter than the model's roof, so its
    /// floor is on or above the roof's plane: no vertical thickness.
    Flatter,
    Outside,
    Masked,
    Inactive,
    /// The cell holds no value of the quality.
    NoValue,
    /// The grid holds no values of the quality.
    NotOnGrid,
}

impl NotCompared {
    /// The reason in words, for the seam, grid and quality of the row.
    pub(crate) fn words(self, seam: &str, grid: &str, quality: &str) -> String {
        match self {
            Self::NoPick => tr!("hole-misfit-reason-no-pick", seam = seam.to_owned()),
            Self::NoTrace => tr!("cmd-thickness-points-reason-no-trace"),
            Self::Repeated => tr!("hole-misfit-reason-repeated"),
            Self::NoFloor => tr!("cmd-thickness-points-reason-no-floor"),
            Self::Overturned => tr!("cmd-thickness-points-reason-overturned"),
            Self::Flatter => tr!("hole-misfit-reason-flatter"),
            Self::Outside => tr!("hole-misfit-reason-outside"),
            Self::Masked => tr!("hole-misfit-reason-masked"),
            Self::Inactive => tr!("hole-misfit-reason-inactive"),
            Self::NoValue => tr!("hole-misfit-reason-no-value", quality = quality.to_owned()),
            Self::NotOnGrid => tr!("hole-misfit-reason-not-on-grid", grid = grid.to_owned(), quality = quality.to_owned()),
        }
    }
}

impl From<PlanMiss> for NotCompared {
    fn from(miss: PlanMiss) -> Self {
        match miss {
            PlanMiss::Outside => Self::Outside,
            PlanMiss::Masked => Self::Masked,
            PlanMiss::Inactive => Self::Inactive,
        }
    }
}

/// One measurement of a grid's misfit.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MisfitRow {
    /// The hole id, or the quality layer's name.
    pub(crate) source: String,
    pub(crate) measure: MisfitMeasure,
    /// Where it was read; `None` without a pick.
    pub(crate) at: Option<DVec2>,
    pub(crate) picked: Option<f64>,
    pub(crate) model: Option<f64>,
    /// The model less the pick.
    pub(crate) result: Result<f64, NotCompared>,
}

/// One quality layer's points, z the value.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MisfitQuality {
    pub(crate) layer: String,
    pub(crate) mark: QualityMark,
    pub(crate) points: Vec<DVec3>,
}

/// One measure's totals: the mean signed difference of the rows compared,
/// and the first largest absolute one, its sign kept, with its row.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MisfitTotals {
    pub(crate) measure: MisfitMeasure,
    pub(crate) compared: usize,
    pub(crate) not_compared: usize,
    pub(crate) mean: Option<f64>,
    pub(crate) largest: Option<(f64, usize)>,
}

/// `value` to three places; one that rounds to zero is 0.000, never -0.000.
pub(crate) fn three_places(value: f64) -> String {
    let text = format!("{value:.3}");
    match text.strip_prefix('-') {
        Some(rest) if rest.bytes().all(|byte| matches!(byte, b'0' | b'.')) => rest.to_owned(),
        _ => text,
    }
}

/// The index of the grid's values of a quality, matched by name.
pub(crate) fn held_quality(grid: &PrismoidGrid, name: &str) -> Option<usize> {
    grid.cell_values.iter().position(|values| same_quality_name(&values.name, name))
}

/// Every row measuring `grid` at the holes, in order, each with its
/// `hole_intercept` (four rows a hole), then at each quality point (one
/// row a point). Fails only when cancelled.
pub(crate) fn misfit_at_holes(
    grid: &PrismoidGrid,
    holes: Vec<(String, Option<Result<Intercept, LeftOut>>)>,
    qualities: &[MisfitQuality],
    cancel: &CancelFlag,
) -> anyhow::Result<Vec<MisfitRow>> {
    let mut rows = Vec::new();
    let mut items = 0usize;
    let mut tick = || -> anyhow::Result<()> {
        items += 1;
        if items % 256 == 0 && cancel.is_cancelled() {
            anyhow::bail!("{}", tr!("common-cancelled"));
        }
        Ok(())
    };
    for (source, found) in holes {
        tick()?;
        hole_rows(grid, source, found, &mut rows);
    }
    for (index, quality) in qualities.iter().enumerate() {
        let held = held_quality(grid, &quality.mark.name);
        for point in &quality.points {
            tick()?;
            let at = point.truncate();
            let model = held.ok_or(NotCompared::NotOnGrid).and_then(|values| {
                let reading = grid.reading_at(at)?;
                grid.cell_values[values].values[reading.cell].filter(|value| value.is_finite()).ok_or(NotCompared::NoValue)
            });
            rows.push(MisfitRow {
                source: quality.layer.clone(),
                measure: MisfitMeasure::Quality(index),
                at: Some(at),
                picked: Some(point.z),
                model: model.ok(),
                result: model.map(|model| model - point.z),
            });
        }
    }
    Ok(rows)
}

fn left_out_reason(reason: LeftOutReason) -> NotCompared {
    match reason {
        LeftOutReason::NoTrace => NotCompared::NoTrace,
        LeftOutReason::NoFloor => NotCompared::NoFloor,
        LeftOutReason::Overturned => NotCompared::Overturned,
        LeftOutReason::OutsideSurface => NotCompared::Outside,
        // Only a measured pair lacks a value.
        LeftOutReason::MissingValue => NotCompared::NoPick,
    }
}

/// A hole's four rows: roof and floor RL each at its own pick, vertical and
/// true thickness at the roof pick, the hole's converted under the grid
/// roof's slope there.
fn hole_rows(grid: &PrismoidGrid, source: String, found: Option<Result<Intercept, LeftOut>>, rows: &mut Vec<MisfitRow>) {
    let row = |measure, at, picked, model, result| MisfitRow {
        source: source.clone(),
        measure,
        at,
        picked,
        model,
        result,
    };
    let intercept = match found {
        Some(Ok(intercept)) if !intercept.repeated => intercept,
        other => {
            let (at, reason) = match other {
                Some(Ok(intercept)) => (Some(intercept.roof.truncate()), NotCompared::Repeated),
                Some(Err(left)) => (None, left_out_reason(left.reason)),
                None => (None, NotCompared::NoPick),
            };
            rows.extend(MisfitMeasure::HOLE.map(|measure| row(measure, at, None, None, Err(reason))));
            return;
        }
    };
    let (roof, at) = (intercept.roof, intercept.roof.truncate());
    let read = grid.reading_at(at).map_err(NotCompared::from);
    let model = read.map(|reading| reading.roof);
    rows.push(row(MisfitMeasure::Roof, Some(at), Some(roof.z), model.ok(), model.map(|model| model - roof.z)));
    match intercept.floor {
        None => rows.push(row(MisfitMeasure::Floor, None, None, None, Err(NotCompared::NoFloor))),
        Some(floor) => {
            let model = grid.reading_at(floor.truncate()).map(|reading| reading.floor).map_err(NotCompared::from);
            rows.push(row(
                MisfitMeasure::Floor,
                Some(floor.truncate()),
                Some(floor.z),
                model.ok(),
                model.map(|model| model - floor.z),
            ));
        }
    }
    let pair = match intercept.floor {
        None => Err(NotCompared::NoFloor),
        Some(floor) if floor.z > roof.z => Err(NotCompared::Overturned),
        // Above the roof's plane through the roof pick the conversion to
        // vertical has no meaning; `true_thickness` would take its size.
        Some(floor) => read.and_then(|reading| {
            let step = floor - roof;
            let above = step.z - reading.roof_slope.dot(step.truncate());
            if above >= 0.0 { Err(NotCompared::Flatter) } else { Ok((floor, reading)) }
        }),
    };
    match pair {
        Err(reason) => rows.extend([MisfitMeasure::Thickness, MisfitMeasure::TrueThickness].map(|measure| row(measure, Some(at), None, None, Err(reason)))),
        Ok((floor, reading)) => {
            let (hole_true, hole_vertical) = true_thickness(roof, floor, reading.roof_slope);
            let vertical = reading.roof - reading.floor;
            let square = vertical / (1.0 + reading.roof_slope.length_squared()).sqrt();
            rows.push(row(MisfitMeasure::Thickness, Some(at), Some(hole_vertical), Some(vertical), Ok(vertical - hole_vertical)));
            rows.push(row(MisfitMeasure::TrueThickness, Some(at), Some(hole_true), Some(square), Ok(square - hole_true)));
        }
    }
}

/// Totals for the hole measures, then for each of `qualities` quality
/// layers, from rows in any order.
pub(crate) fn misfit_totals(rows: &[MisfitRow], qualities: usize) -> Vec<MisfitTotals> {
    let measures = MisfitMeasure::HOLE.into_iter().chain((0..qualities).map(MisfitMeasure::Quality));
    let mut totals: Vec<MisfitTotals> = measures
        .map(|measure| MisfitTotals {
            measure,
            compared: 0,
            not_compared: 0,
            mean: None,
            largest: None,
        })
        .collect();
    let mut sums = vec![0.0; totals.len()];
    for (index, row) in rows.iter().enumerate() {
        let slot = row.measure.slot();
        let Some(total) = totals.get_mut(slot) else {
            continue;
        };
        match row.result {
            Ok(difference) => {
                total.compared += 1;
                sums[slot] += difference;
                if total.largest.is_none_or(|(largest, _)| difference.abs() > largest.abs()) {
                    total.largest = Some((difference, index));
                }
            }
            Err(_) => total.not_compared += 1,
        }
    }
    for (total, sum) in totals.iter_mut().zip(sums) {
        total.mean = (total.compared > 0).then_some(sum / total.compared as f64);
    }
    totals
}
