//! Rehabilitating a dump: reshaping it, holding its volume, into one even
//! slope down to the ground.
//!
//! The reshaped landform is designed as any dump built down from its top
//! is: from a ring at its top RL, stepping out at the slope asked until it
//! meets the ground. Its top ring is the dump's own section at that RL, set
//! out (or in) as far as makes the reshaped landform hold what the dump
//! does: a flatter slope spreads the dump wider and its top narrower, a
//! lower top wider still. What it takes is the earthworks between the two:
//! ground cut from the dump's shoulders, and placed beyond its toes.

use anyhow::Result;
use glam::{DVec2, DVec3};

use super::triangulation::solid_between::{measured_volume_between, plan_footprint_rings};
use crate::{
    i18n::tr,
    model::{
        formats::mesh_data,
        pit_design::{
            self, Base, Design, DesignString, Direction, Kind, Spec, Stop, StringKind,
            evaluation::{RehabFigures, RehabSettings},
            ring::{self, EdgeOffset},
        },
        spatial::TriangleBvh,
    },
    ui::state::SolidRegion,
};

/// How near the dump's own volume the reshaped landform's must come, as a
/// share of it.
const VOLUME_TOLERANCE: f64 = 1.0e-4;

/// How many times the top ring's offset is halved, at most, to match the
/// volume.
const SEARCH_STEPS: usize = 40;

/// How far the top ring is first set out or in, to find which way it must
/// go, in plan.
const FIRST_STEP: f64 = 10.0;

/// The ground, for the engine to daylight against.
struct Ground<'a> {
    mesh: &'a mesh_data::Triangulation,
    spatial: &'a TriangleBvh,
}

impl Ground<'_> {
    fn rl(&self, point: DVec2) -> Option<f64> {
        let bounds = self.mesh.bounds();
        if point.x < bounds.min.x || point.x > bounds.max.x || point.y < bounds.min.y || point.y > bounds.max.y {
            return None;
        }
        let above = bounds.max.z + ((bounds.max.z - bounds.min.z).abs() * 1.0e-6).max(1.0);
        self.spatial.ray_hit(self.mesh, point.extend(above), -DVec3::Z).map(|hit| hit.z)
    }
}

/// A dump's footprint in plan: each sheet of a closed solid covers it once,
/// its top and its bottom, so half the plan area of all its faces.
fn footprint(mesh: &mesh_data::Triangulation) -> f64 {
    let vertices = mesh.vertices();
    mesh.face_vertex_indices_iter()
        .map(|face| {
            let [a, b, c] = face.map(|index| DVec2::new(vertices[index].x, vertices[index].y));
            ((b - a).perp_dot(c - a) * 0.5).abs()
        })
        .sum::<f64>()
        * 0.5
}

/// The reshaped landform from `top`, a ring at RL `rl`: its design and its
/// surface, or `None` where the ring leaves it no room.
fn reshaped(top: &[DVec2], rl: f64, settings: &RehabSettings, ground: &Ground<'_>) -> Result<Option<(Design, mesh_data::Triangulation)>> {
    let points: Vec<DVec3> = top.iter().map(|point| point.extend(rl)).collect();
    let Some(base) = Base::from_points(&points) else {
        return Ok(None);
    };
    let spec = Spec {
        kind: Kind::Dump,
        direction: Direction::Down,
        bench_height: settings.contour,
        face_angle: settings.slope,
        berm_width: 0.0,
        rules: pit_design::BenchRules::default(),
        wall_radius: 0.0,
        ramps: Vec::new(),
        stop: Stop::Daylight,
    };
    let sample = |point: DVec2| ground.rl(point);
    let design = match pit_design::design(&base, &spec, Some(&sample), &|| false) {
        Ok(design) => design,
        Err(problem) => anyhow::bail!(crate::ui::dialogs::pit_design::problem_text(problem)),
    };
    let strings: Vec<(Vec<DVec3>, bool)> = design.strings.iter().map(|string| (string.points.clone(), string.closed)).collect();
    let surface = super::triangulation::creation::surface_through_strings(strings)?;
    Ok(Some((design, surface)))
}

/// `ring` set out by `distance` (in where negative), as its largest piece.
fn set_out(ring: &[DVec2], distance: f64) -> Option<Vec<DVec2>> {
    if distance == 0.0 {
        return Some(ring.to_vec());
    }
    ring::offset(ring, &vec![EdgeOffset::even(distance); ring.len()]).into_iter().next()
}

/// Rehabilitate the dump whose closed solid is `dump`, designed as `design`
/// with the surface `design_surface`, against `ground`.
pub(super) fn rehabilitate(
    design: &Design,
    design_surface: &mesh_data::Triangulation,
    dump: &mesh_data::Triangulation,
    dump_volume: f64,
    settings: &RehabSettings,
    (ground_mesh, ground_spatial): (&mesh_data::Triangulation, &TriangleBvh),
    cancel: &crate::app::jobs::CancelFlag,
) -> Result<RehabFigures> {
    anyhow::ensure!(
        settings.slope > 0.0 && settings.slope < 90.0 && settings.contour > 0.0,
        tr!("pit-design-rehab-bad-settings")
    );
    let ground = Ground {
        mesh: ground_mesh,
        spatial: ground_spatial,
    };
    // Its top: as high as it stands, or as high as it may.
    let highest = design.levels.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let rl = settings.top.map_or(highest, |top| top.min(highest));
    // The dump's own section a hair under its top, its largest ring.
    let section = plan_footprint_rings(dump, rl - 1.0e-3)
        .into_iter()
        .map(|ring| ring::normalised(ring.iter().map(|vertex| DVec2::new(vertex.x, vertex.y))))
        .max_by(|a, b| ring::signed_area(a).total_cmp(&ring::signed_area(b)))
        .filter(|ring| ring.len() >= 3)
        .ok_or_else(|| anyhow::anyhow!(tr!("pit-design-rehab-no-top")))?;
    // How much more the landform from a top ring set out `distance` holds
    // than the dump; no room left is as little as can be.
    type Shaped = Option<(Design, mesh_data::Triangulation)>;
    let excess = |distance: f64| -> Result<(f64, Shaped)> {
        anyhow::ensure!(!cancel.is_cancelled(), crate::app::jobs::Cancelled);
        let Some(top) = set_out(&section, distance) else {
            return Ok((-dump_volume, None));
        };
        let Some((reshaped, surface)) = reshaped(&top, rl, settings, &ground)? else {
            return Ok((-dump_volume, None));
        };
        let volume = measured_volume_between(&surface, ground_mesh, SolidRegion::Fill)?.unwrap_or(0.0);
        Ok((volume - dump_volume, Some((reshaped, surface))))
    };
    // The nearest to the dump's volume found so far.
    let mut best: Option<(f64, (Design, mesh_data::Triangulation))> = None;
    let mut keep = |found: f64, shaped: Shaped| {
        if let Some(shaped) = shaped
            && best.as_ref().is_none_or(|(nearest, _)| found.abs() < nearest.abs())
        {
            best = Some((found, shaped));
        }
    };
    let close = |found: f64| found.abs() <= dump_volume * VOLUME_TOLERANCE;
    // The volume grows as the top ring is set out: bracket the offset that
    // holds the dump's, then halve the bracket.
    let (at_section, shaped) = excess(0.0)?;
    keep(at_section, shaped);
    if !close(at_section) {
        let outward = at_section < 0.0;
        let (mut low, mut high) = (0.0, 0.0);
        let mut step = FIRST_STEP;
        loop {
            let probe = if outward { step } else { -step };
            let (found, shaped) = excess(probe)?;
            keep(found, shaped);
            if (found > 0.0) == outward {
                if outward {
                    high = probe
                } else {
                    low = probe
                }
                break;
            }
            if outward {
                low = probe
            } else {
                high = probe
            }
            step *= 2.0;
            anyhow::ensure!(step < 1.0e5, tr!("pit-design-rehab-no-room"));
        }
        for _ in 0..SEARCH_STEPS {
            let middle = 0.5 * (low + high);
            let (found, shaped) = excess(middle)?;
            keep(found, shaped);
            if close(found) {
                break;
            }
            if found > 0.0 { high = middle } else { low = middle }
        }
    }
    // Still far off: the slope is too flat to hold the dump from this top,
    // however small its top is left.
    let (found, (reshaped, surface)) = best.ok_or_else(|| anyhow::anyhow!(tr!("pit-design-rehab-too-flat")))?;
    anyhow::ensure!(found.abs() <= dump_volume * 0.01, tr!("pit-design-rehab-too-flat"));
    let solid = super::triangulation::solid_between::build_solid_mesh(
        &surface,
        ground_mesh,
        SolidRegion::Fill,
        super::triangulation::solid_between::BuiltFor::Measure,
        &crate::model::progress::Progress::new().phase(0.0, 1.0),
    )?;
    // The earthworks: each landform set into the ground round both, and the
    // solid between them; where the dump's lies below the reshaped one's,
    // ground placed, above, ground cut.
    let (dump_bounds, rehab_bounds) = (dump.bounds(), solid.mesh.bounds());
    let margin = DVec2::splat(super::pit_evaluation::LANDFORM_MARGIN);
    let min = DVec2::new(dump_bounds.min.x.min(rehab_bounds.min.x), dump_bounds.min.y.min(rehab_bounds.min.y)) - margin;
    let max = DVec2::new(dump_bounds.max.x.max(rehab_bounds.max.x), dump_bounds.max.y.max(rehab_bounds.max.y)) + margin;
    let cropped = super::pit_evaluation::cropped(ground_mesh, (min, max))?;
    let spatial = TriangleBvh::build(&cropped);
    let include = super::triangulation::include::landform_with;
    let as_designed = include(&cropped, &spatial, design_surface, &crate::model::progress::Progress::new().phase(0.0, 1.0))?;
    let as_reshaped = include(&cropped, &spatial, &surface, &crate::model::progress::Progress::new().phase(0.0, 1.0))?;
    let fill = measured_volume_between(&as_designed, &as_reshaped, SolidRegion::Cut)?.unwrap_or(0.0);
    let cut = measured_volume_between(&as_designed, &as_reshaped, SolidRegion::Fill)?.unwrap_or(0.0);
    Ok(RehabFigures {
        strings: reshaped
            .strings
            .into_iter()
            .map(|string| DesignString {
                kind: StringKind::Rehab,
                ..string
            })
            .collect(),
        top: rl,
        volume: solid.volume,
        cut,
        fill,
        footprint: [footprint(dump), footprint(&solid.mesh)],
        problem: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A steep dump on flat ground, reshaped flatter: it holds its volume,
    /// what is cut from it is what is placed beyond it, and it covers more
    /// ground; with its top held lower, it spreads further still.
    #[test]
    fn a_rehabilitated_dump_holds_its_volume_and_spreads() {
        let corners = [(-500.0, -500.0), (500.0, -500.0), (500.0, 500.0), (-500.0, 500.0)];
        let ground =
            mesh_data::Triangulation::from_vertices_and_faces(corners.iter().map(|&(x, y)| mesh_data::Vertex::new(x, y, 0.0)).collect(), vec![[0, 1, 2], [0, 2, 3]]).unwrap();
        let spatial = TriangleBvh::build(&ground);
        let toe: Vec<DVec3> = (0..48)
            .map(|i| (DVec2::from_angle(f64::from(i) * std::f64::consts::TAU / 48.0) * 150.0).extend(0.0))
            .collect();
        let spec = Spec {
            kind: Kind::Dump,
            direction: Direction::Up,
            bench_height: 15.0,
            face_angle: 37.0,
            berm_width: 10.0,
            rules: pit_design::BenchRules::default(),
            wall_radius: 0.0,
            ramps: Vec::new(),
            stop: Stop::Benches(3),
        };
        let design = pit_design::design(&Base::from_points(&toe).unwrap(), &spec, None, &|| false).unwrap();
        let strings = design.strings.iter().map(|string| (string.points.clone(), string.closed)).collect();
        let surface = crate::app::commands::triangulation::creation::surface_through_strings(strings).unwrap();
        let progress = crate::model::progress::Progress::new();
        let dump = super::super::triangulation::solid_between::build_solid_mesh(
            &surface,
            &ground,
            SolidRegion::Fill,
            super::super::triangulation::solid_between::BuiltFor::Measure,
            &progress.phase(0.0, 1.0),
        )
        .unwrap();
        let cancel = crate::app::jobs::CancelFlag::default();
        let mut spread = 0.0;
        for top in [None, Some(35.0)] {
            let settings = RehabSettings { slope: 18.0, top, contour: 5.0 };
            let rehab = rehabilitate(&design, &surface, &dump.mesh, dump.volume, &settings, (&ground, &spatial), &cancel).unwrap();
            assert!(
                (rehab.volume - dump.volume).abs() < dump.volume * 1.0e-3,
                "{top:?}: {} held for {}",
                rehab.volume,
                dump.volume
            );
            assert!((rehab.cut - rehab.fill).abs() < rehab.cut * 0.01, "{top:?}: cut {} placed {}", rehab.cut, rehab.fill);
            assert!(rehab.footprint[1] > rehab.footprint[0] && rehab.footprint[1] > spread, "{top:?}: {:?}", rehab.footprint);
            assert!(rehab.strings.iter().all(|string| string.kind == StringKind::Rehab));
            spread = rehab.footprint[1];
        }
    }
}
