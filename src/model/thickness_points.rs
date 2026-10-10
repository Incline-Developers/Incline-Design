//! Thickness points: the true thickness of one working section at each hole
//! that holds it, and at each measured roof and floor pair, measured square
//! to the bedding of a reference surface.
//!
//! True thickness is the roof-to-floor vector taken on the bedding normal,
//! one formula for every hole whatever its angle to the seam [Dennison 1968;
//! Heckscher & Saunders, "Precision in the dip"]. The bedding is the
//! reference surface's slope at the roof, read off its own grid nodes, so a
//! surface from a saved project measures as well as one built this session.

use glam::{DVec2, DVec3};

use crate::{
    app::jobs::CancelFlag,
    i18n::tr,
    model::{
        Command,
        drill_hole::{DrillHole, DrillHoleDataset, DrillHoleId, DrillValue, section_run},
        grid_surface::GridSurface,
        rbf::Dip,
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
