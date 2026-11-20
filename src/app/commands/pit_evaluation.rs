//! Measuring pit designs: what each takes out of the ground (or a dump puts
//! on it), bench by bench, and what a block model holds of that.
//!
//! The design's strings make a surface as Create Triangulation would, which
//! closes against the topography into the solid between them: cut where the
//! design lies below the ground for a pit, fill above it for a dump. The
//! solid is cut at the design's RLs into its benches, and each bench is
//! measured in the block model chosen by the project's reserve fields, as
//! Planning measures its solids.
//!
//! Against an optimisation shell, the design and the shell are each set
//! into the topography as a landform, and the solid between the two
//! landforms is the difference: where the design's lies below the shell's,
//! ground the design takes beyond the shell; where above, ground inside the
//! shell it leaves. Built from the same topography, the two landforms are
//! the same ground beyond both, and part only where either does.
//!
//! [`App::refresh_pit_evaluations`] measures, in the background, every
//! design whose measure is older than it, once the design itself is current
//! and what it is measured against is loaded. Like the design, the measure
//! is derived: kept on the object with what it was measured from, and no
//! edit of its own.

use std::sync::Arc;

use super::pit_design::pit_parts;
use crate::{
    app::{App, jobs::JobKey},
    i18n::tr,
    model::{
        Object, ObjectId, ReserveField,
        block_model::{BlockBoundsSource, ReserveFieldMapping},
        formats::{block_model_data::BlockModelData, mesh_data},
        pit_design::{
            Design, StringKind,
            evaluation::{BenchFigures, Evaluation, Figures, Measured, ShellFigures, Waiting},
        },
        solid_reserves::{ReserveBand, ReservePartition, ReservePiece, ReserveTotals},
        spatial::TriangleBvh,
    },
    ui::state::SolidRegion,
};

/// What a design is measured with, ready for a worker.
struct Inputs {
    design: Design,
    topography: Arc<mesh_data::Triangulation>,
    model: Option<ModelInputs>,
    /// The optimisation shell it is set against, if one is chosen.
    shell: Option<Arc<mesh_data::Triangulation>>,
}

/// The block model a design is measured in, and how.
struct ModelInputs {
    data: BlockModelData,
    blocks: Arc<BlockBoundsSource>,
    mapping: Vec<ReserveFieldMapping>,
    fields: Vec<ReserveField>,
}

/// What a design would be measured against now, and the fingerprint of it:
/// all held where it is, until a worker needs a copy.
struct Plan<'a> {
    key: u64,
    design: &'a Design,
    topography: &'a crate::model::triangulation::OpenTriangulation,
    model: Option<&'a crate::model::block_model::OpenBlockModel>,
    fields: Vec<ReserveField>,
    shell: Option<&'a crate::model::triangulation::OpenTriangulation>,
}

impl Plan<'_> {
    /// A worker's copy of it.
    fn inputs(&self) -> Inputs {
        Inputs {
            design: self.design.clone(),
            topography: self.topography.mesh.clone(),
            model: self.model.map(|model| ModelInputs {
                data: model.model.clone(),
                blocks: model.blocks.clone(),
                mapping: model.reserve_mapping.clone(),
                fields: self.fields.clone(),
            }),
            shell: self.shell.map(|shell| shell.mesh.clone()),
        }
    }
}

/// FNV-1a over `parts`, the same on every build and platform.
fn fingerprint(parts: &[&[u8]]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for part in parts {
        for byte in (part.len() as u64).to_le_bytes().iter().chain(part.iter()) {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
    }
    hash
}

impl App<'_> {
    /// What `object`'s design would be measured with now, and the
    /// fingerprint of it; or why it cannot be yet.
    fn pit_evaluation_plan<'b>(&'b self, object: &'b Object) -> Result<Plan<'b>, Waiting> {
        let (verts, pit) = pit_parts(object).ok_or(Waiting::Design)?;
        let settings = pit.settings();
        let ground = self.pit_ground_version(&settings);
        if !pit.is_fresh(verts, ground) {
            return Err(Waiting::Design);
        }
        let design = pit.design().ok_or(Waiting::Design)?;
        let derived = pit.derived.as_ref().ok_or(Waiting::Design)?;
        let surface = settings
            .topography
            .and_then(|id| self.triangulations.iter().find(|surface| surface.id == id))
            .ok_or(Waiting::NoTopography)?;
        if !surface.state.loaded || surface.state.deferred.is_some() {
            return Err(Waiting::NotLoaded(surface.name.clone()));
        }
        let fields = self.workspace.active_document().map(|document| document.reserve_fields().to_vec()).unwrap_or_default();
        let model = match settings.evaluation.block_model {
            None => None,
            Some(id) => {
                let Some(model) = self.block_models.iter().find(|model| model.id == id) else {
                    return Err(Waiting::NotLoaded(tr!("pit-design-evaluate-missing-model")));
                };
                // Every column it maps must be in memory to be read.
                let resident = model
                    .reserve_mapping
                    .iter()
                    .filter_map(|entry| entry.source.column())
                    .all(|name| model.model.shared_numeric_values(name).is_some());
                if !model.state.loaded || !resident {
                    return Err(Waiting::NotLoaded(model.name.clone()));
                }
                Some(model)
            }
        };
        // The shell, for a pit: a dump has none to be set against.
        let shell = match settings.evaluation.shell.filter(|_| settings.kind == crate::model::pit_design::Kind::Pit) {
            None => None,
            Some(id) => {
                let Some(shell) = self.triangulations.iter().find(|surface| surface.id == id) else {
                    return Err(Waiting::NotLoaded(tr!("pit-design-evaluate-missing-shell")));
                };
                if !shell.state.loaded || shell.state.deferred.is_some() {
                    return Err(Waiting::NotLoaded(shell.name.clone()));
                }
                Some(shell)
            }
        };
        let (ground_bits, surface_bits) = (
            derived.ground.map_or([0; 2], |version| <[u64; 2]>::from(version.0.as_u64_pair())),
            <[u64; 2]>::from(surface.geometry.0.as_u64_pair()),
        );
        let mut parts: Vec<Vec<u8>> = vec![
            derived.inputs.to_le_bytes().to_vec(),
            ground_bits.iter().flat_map(|bits| bits.to_le_bytes()).collect(),
            surface.id.0.to_le_bytes().to_vec(),
            surface_bits.iter().flat_map(|bits| bits.to_le_bytes()).collect(),
        ];
        if let Some(shell) = shell {
            parts.push(shell.id.0.to_le_bytes().to_vec());
            parts.push(<[u64; 2]>::from(shell.geometry.0.as_u64_pair()).iter().flat_map(|bits| bits.to_le_bytes()).collect());
        }
        if let Some(model) = model {
            parts.push(model.id.0.to_le_bytes().to_vec());
            parts.push((model.model.metadata.n_blocks as u64).to_le_bytes().to_vec());
            parts.push(serde_json::to_vec(&fields).unwrap_or_default());
            parts.push(serde_json::to_vec(&model.reserve_mapping).unwrap_or_default());
        }
        let key = fingerprint(&parts.iter().map(Vec::as_slice).collect::<Vec<_>>());
        Ok(Plan {
            key,
            design,
            topography: surface,
            model,
            fields,
            shell,
        })
    }

    /// Why the design open in Pit Design is not measured as it stands, or
    /// `None` where its measure is current.
    pub(crate) fn pit_evaluation_waiting(&self, id: ObjectId) -> Option<Waiting> {
        let object = self.operand_object(id)?;
        let (_, pit) = pit_parts(object)?;
        match self.pit_evaluation_plan(object) {
            Err(waiting) => Some(waiting),
            Ok(plan) if pit.evaluation.as_ref().is_some_and(|evaluation| evaluation.key == plan.key) => None,
            Ok(_) => Some(Waiting::Measuring),
        }
    }

    /// Measure, in the background, every pit design whose measure is older
    /// than it and that can be measured now.
    pub(crate) fn refresh_pit_evaluations(&mut self) {
        let Some(project) = self.workspace.active_project() else {
            return;
        };
        let checked = {
            use std::hash::{DefaultHasher, Hash, Hasher};
            let mut hasher = DefaultHasher::new();
            project.runtime_id.hash(&mut hasher);
            project.project.document.revision().hash(&mut hasher);
            for surface in &self.triangulations {
                (surface.id, surface.geometry, surface.state.loaded, surface.state.deferred.is_some()).hash(&mut hasher);
            }
            for model in &self.block_models {
                (model.id, model.state.loaded, model.state.deferred.is_some(), model.model.metadata.n_blocks).hash(&mut hasher);
                serde_json::to_vec(&model.reserve_mapping).unwrap_or_default().hash(&mut hasher);
            }
            serde_json::to_vec(project.project.document.reserve_fields()).unwrap_or_default().hash(&mut hasher);
            self.editor.pit_evaluations.len().hash(&mut hasher);
            hasher.finish()
        };
        if self.editor.pit_evaluation_checked == Some(checked) {
            return;
        }
        self.editor.pit_evaluation_checked = Some(checked);
        let stale: Vec<(ObjectId, u64, Inputs)> = project
            .project
            .document
            .objects()
            .iter()
            .filter_map(|object| {
                let (_, pit) = pit_parts(object)?;
                let plan = self.pit_evaluation_plan(object).ok()?;
                let key = plan.key;
                let current = pit.evaluation.as_ref().is_some_and(|evaluation| evaluation.key == key);
                (!current && self.editor.pit_evaluations.get(&object.id()) != Some(&key)).then(|| (object.id(), key, plan.inputs()))
            })
            .collect();
        for (id, key, inputs) in stale {
            self.editor.pit_evaluations.insert(id, key);
            self.spawn_job_reporting_progress(
                tr!("pit-design-measuring"),
                vec![JobKey::PitDesignEvaluation(id)],
                move |cancel, progress| {
                    Ok(match measure(&inputs, cancel, progress) {
                        Ok(mut evaluation) => {
                            evaluation.key = key;
                            evaluation
                        }
                        Err(error) if error.is::<crate::app::jobs::Cancelled>() => return Err(error),
                        Err(error) => Evaluation::failed(key, format!("{error:#}")),
                    })
                },
                move |app, result| {
                    if app.editor.pit_evaluations.get(&id) == Some(&key) {
                        app.editor.pit_evaluations.remove(&id);
                    }
                    let Ok(evaluation) = result else {
                        return;
                    };
                    // Only onto the design as it was measured: one changed
                    // since is measured again for what it is now.
                    let now = app.operand_object(id).and_then(|object| app.pit_evaluation_plan(object).ok()).map(|plan| plan.key);
                    if now != Some(key) {
                        return;
                    }
                    let Some(document) = app.workspace.active_document_mut() else {
                        return;
                    };
                    let Some(mut object) = document.get_object(id).cloned() else {
                        return;
                    };
                    if let Object::PitDesign { pit, .. } = &mut object {
                        pit.evaluation = Some(evaluation);
                    }
                    document.replace_object(object);
                    app.invalidate_geometry();
                },
            );
        }
    }
}

/// Measure a design: its surface closed against the topography, cut into
/// its benches, each measured in the block model if there is one. The key
/// is left for the caller.
fn measure(inputs: &Inputs, cancel: &crate::app::jobs::CancelFlag, progress: &crate::model::progress::Progress) -> anyhow::Result<Evaluation> {
    let design = &inputs.design;
    let strings: Vec<(Vec<glam::DVec3>, bool)> = design
        .strings
        .iter()
        .filter(|string| string.kind != StringKind::RampCentreline)
        .map(|string| (string.points.clone(), string.closed))
        .collect();
    let surface = super::triangulation::creation::surface_through_strings(strings)?;
    anyhow::ensure!(!cancel.is_cancelled(), crate::app::jobs::Cancelled);
    let region = if design.dump { SolidRegion::Fill } else { SolidRegion::Cut };
    let solid = super::triangulation::solid_between::build_solid_mesh(&surface, &inputs.topography, region, "", &progress.phase(0.0, 0.3))?;
    anyhow::ensure!(!cancel.is_cancelled(), crate::app::jobs::Cancelled);

    // The benches, bottom up, the lowest and highest reaching as far as the
    // solid does: ground above the last ring where the design stops short
    // of daylight, say.
    let mut levels = design.levels.clone();
    levels.sort_by(f64::total_cmp);
    levels.dedup_by(|a, b| (*a - *b).abs() < 1.0e-6);
    let bounds = solid.mesh.bounds();
    let mut ranges: Vec<(f64, f64)> = levels.windows(2).map(|pair| (pair[0], pair[1])).collect();
    if let (Some(first), Some(last)) = (ranges.first().copied(), ranges.last().copied()) {
        if bounds.min.z < first.0 - 1.0e-6 {
            ranges[0].0 = bounds.min.z - 1.0e-3;
        }
        if bounds.max.z > last.1 + 1.0e-6 {
            let end = ranges.len() - 1;
            ranges[end].1 = bounds.max.z + 1.0e-3;
        }
    } else {
        ranges.push((bounds.min.z - 1.0e-3, bounds.max.z + 1.0e-3));
    }
    let slabs = super::triangulation::solid_between::slabs_between_elevations(&solid.mesh, &ranges, Some(cancel))?;
    let mut benches = Vec::new();
    let mut bands = Vec::new();
    for ((base, top), (vertices, faces)) in ranges.iter().copied().zip(slabs) {
        if faces.is_empty() {
            continue;
        }
        let (vertices, faces) = crate::model::triangulation::weld_identical_points(vertices, faces);
        let Ok(mesh) = mesh_data::Triangulation::from_vertices_and_faces(vertices, faces) else {
            continue;
        };
        let mesh = Arc::new(mesh);
        benches.push(BenchFigures {
            base,
            top,
            figures: Figures {
                volume: super::solids_view::mesh_volume(&mesh),
                measured: None,
            },
        });
        bands.push(ReserveBand {
            base,
            top,
            pieces: vec![ReservePiece {
                spatial: Arc::new(TriangleBvh::deferred(mesh.clone())),
                mesh,
                slot: bands.len(),
            }],
        });
    }
    progress.set_fraction(0.4);
    let mut total = Figures {
        volume: solid.volume,
        measured: None,
    };
    if let Some(model) = &inputs.model {
        let partition = ReservePartition { slots: bands.len(), bands };
        let outcome = crate::model::solid_reserves::compute(&model.data, &model.blocks, &model.mapping, &model.fields, &partition, false, cancel, progress)?;
        for (bench, totals) in benches.iter_mut().zip(&outcome.totals) {
            bench.figures.measured = Some(Measured::of(totals, &model.fields));
        }
        let mut all: Option<ReserveTotals> = None;
        for totals in &outcome.totals {
            match &mut all {
                Some(all) => all.merge(totals),
                None => all = Some(totals.clone()),
            }
        }
        total.measured = all.map(|all| Measured::of(&all, &model.fields));
    }
    let shell = inputs.shell.as_ref().map(|shell| match against_shell(inputs, &surface, shell, cancel, progress) {
        Ok(figures) => Ok(figures),
        Err(error) if error.is::<crate::app::jobs::Cancelled>() => Err(error),
        Err(error) => Ok(ShellFigures {
            problem: Some(format!("{error:#}")),
            ..ShellFigures::default()
        }),
    });
    let shell = shell.transpose()?;
    progress.set_fraction(1.0);
    Ok(Evaluation {
        key: 0,
        problem: None,
        total,
        benches,
        shell,
    })
}

/// How far round both the design and the shell the topography is kept to
/// set them into: well clear of either, so the two landforms are the same
/// ground at their edges.
const LANDFORM_MARGIN: f64 = 50.0;

/// The faces of `mesh` within `(min, max)` in plan.
fn cropped(mesh: &mesh_data::Triangulation, (min, max): (glam::DVec2, glam::DVec2)) -> anyhow::Result<mesh_data::Triangulation> {
    let vertices = mesh.vertices();
    let mut index_of = vec![u32::MAX; vertices.len()];
    let (mut kept_vertices, mut kept_faces) = (Vec::new(), Vec::new());
    for face in mesh.face_vertex_indices_iter() {
        let corners = face.map(|index| vertices[index]);
        let (low, high) = super::triangulation::cuts::triangle_xy_bounds(corners);
        if high.x < min.x || high.y < min.y || low.x > max.x || low.y > max.y {
            continue;
        }
        kept_faces.push(face.map(|index| {
            if index_of[index] == u32::MAX {
                index_of[index] = kept_vertices.len() as u32;
                kept_vertices.push(vertices[index]);
            }
            index_of[index]
        }));
    }
    Ok(mesh_data::Triangulation::from_vertices_and_faces(kept_vertices, kept_faces)?)
}

/// A closed solid's floor and walls: every face but those its top is made
/// of, which face up out of it. `None` for an open surface: it is its own.
fn floor_and_walls(mesh: &mesh_data::Triangulation) -> anyhow::Result<Option<mesh_data::Triangulation>> {
    let faces: Vec<[u32; 3]> = mesh.face_vertex_indices_iter().map(|face| face.map(|index| index as u32)).collect();
    let vertices = mesh.vertices();
    if super::triangulation::solid_between::open_edge_report(vertices, &faces).is_some() {
        return Ok(None);
    }
    let point = |index: u32| {
        let vertex = vertices[index as usize];
        glam::DVec3::new(vertex.x, vertex.y, vertex.z)
    };
    let normal = |face: &[u32; 3]| (point(face[1]) - point(face[0])).cross(point(face[2]) - point(face[0]));
    // Wound so its faces look out of it, or the other way.
    let origin = point(faces[0][0]);
    let outward = faces.iter().map(|face| (point(face[0]) - origin).dot(normal(face))).sum::<f64>() >= 0.0;
    let kept: Vec<[u32; 3]> = faces
        .iter()
        .copied()
        .filter(|face| {
            let up = normal(face).z * if outward { 1.0 } else { -1.0 };
            up <= 1.0e-9 * normal(face).length()
        })
        .collect();
    Ok(Some(mesh_data::Triangulation::from_vertices_and_faces(vertices.to_vec(), kept)?))
}

/// The design, by its surface, set against the optimisation `shell`: what
/// it takes beyond the shell and what of it it leaves, each measured. Each
/// is set into the topography by its floor and walls, as the Include tool
/// sets a pit shell: the topography round it kept, the ground over it taken
/// away.
fn against_shell(
    inputs: &Inputs,
    design: &mesh_data::Triangulation,
    shell: &mesh_data::Triangulation,
    cancel: &crate::app::jobs::CancelFlag,
    progress: &crate::model::progress::Progress,
) -> anyhow::Result<ShellFigures> {
    use super::triangulation::solid_between::{build_solid_mesh, no_volume};
    let own = floor_and_walls(shell)?;
    let shell = own.as_ref().unwrap_or(shell);
    // The ground round both, set with each in turn.
    let (design_bounds, shell_bounds) = (design.bounds(), shell.bounds());
    let min = glam::DVec2::new(design_bounds.min.x.min(shell_bounds.min.x), design_bounds.min.y.min(shell_bounds.min.y)) - glam::DVec2::splat(LANDFORM_MARGIN);
    let max = glam::DVec2::new(design_bounds.max.x.max(shell_bounds.max.x), design_bounds.max.y.max(shell_bounds.max.y)) + glam::DVec2::splat(LANDFORM_MARGIN);
    let ground = cropped(&inputs.topography, (min, max))?;
    let spatial = TriangleBvh::build(&ground);
    let include = super::triangulation::include::landform_with;
    let with_design = include(&ground, &spatial, design, &progress.phase(0.5, 0.6))?;
    anyhow::ensure!(!cancel.is_cancelled(), crate::app::jobs::Cancelled);
    let with_shell = include(&ground, &spatial, shell, &progress.phase(0.6, 0.7))?;
    anyhow::ensure!(!cancel.is_cancelled(), crate::app::jobs::Cancelled);
    // Below the shell's landform, the design takes more; above, it leaves
    // some. Neither at all is none.
    let between = |region: SolidRegion, phase| match build_solid_mesh(&with_design, &with_shell, region, "", &phase) {
        Ok(solid) => Ok(Some(solid)),
        Err(error) if error.chain().any(|cause| cause.to_string() == no_volume(region)) => Ok(None),
        Err(error) => Err(error),
    };
    let gained = between(SolidRegion::Cut, progress.phase(0.75, 0.8))?;
    let left = between(SolidRegion::Fill, progress.phase(0.8, 0.85))?;
    let mut figures = ShellFigures {
        gained: Figures {
            volume: gained.as_ref().map_or(0.0, |solid| solid.volume),
            measured: None,
        },
        left: Figures {
            volume: left.as_ref().map_or(0.0, |solid| solid.volume),
            measured: None,
        },
        problem: None,
    };
    if let Some(model) = &inputs.model {
        // Apart in space, so two pieces of one band spanning both.
        let pieces: Vec<ReservePiece> = [&gained, &left]
            .into_iter()
            .enumerate()
            .filter_map(|(slot, solid)| {
                let mesh = solid.as_ref()?.mesh.clone();
                Some(ReservePiece {
                    spatial: Arc::new(TriangleBvh::deferred(mesh.clone())),
                    mesh,
                    slot,
                })
            })
            .collect();
        let (low, high) = pieces.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), piece| {
            let bounds = piece.mesh.bounds();
            (low.min(bounds.min.z), high.max(bounds.max.z))
        });
        let partition = ReservePartition {
            slots: 2,
            bands: if pieces.is_empty() {
                Vec::new()
            } else {
                vec![ReserveBand {
                    base: low - 1.0e-3,
                    top: high + 1.0e-3,
                    pieces,
                }]
            },
        };
        let outcome = crate::model::solid_reserves::compute(&model.data, &model.blocks, &model.mapping, &model.fields, &partition, false, cancel, progress)?;
        figures.gained.measured = outcome.totals.first().map(|totals| Measured::of(totals, &model.fields));
        figures.left.measured = outcome.totals.get(1).map(|totals| Measured::of(totals, &model.fields));
    }
    Ok(figures)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A closed box, wound either way, gives up its top and keeps its floor
    /// and walls; an open surface is left as it is.
    #[test]
    fn a_closed_shell_keeps_its_floor_and_walls() {
        let corners = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        let mut vertices: Vec<mesh_data::Vertex> = corners.iter().map(|&(x, y)| mesh_data::Vertex::new(x, y, 0.0)).collect();
        vertices.extend(corners.iter().map(|&(x, y)| mesh_data::Vertex::new(x, y, 5.0)));
        // Outward: the floor looks down, the top up, the walls out.
        let mut faces: Vec<[u32; 3]> = vec![[0, 2, 1], [0, 3, 2], [4, 5, 6], [4, 6, 7]];
        for side in 0..4u32 {
            let (a, b) = (side, (side + 1) % 4);
            faces.push([a, b, b + 4]);
            faces.push([a, b + 4, a + 4]);
        }
        for flipped in [false, true] {
            let wound: Vec<[u32; 3]> = faces.iter().map(|&[a, b, c]| if flipped { [a, c, b] } else { [a, b, c] }).collect();
            let mesh = mesh_data::Triangulation::from_vertices_and_faces(vertices.clone(), wound).unwrap();
            let kept = floor_and_walls(&mesh).unwrap().expect("a closed box has a top to drop");
            assert_eq!(kept.face_count(), 10, "flipped {flipped}");
            assert!(
                kept.face_vertex_indices_iter().all(|face| face.iter().any(|&index| kept.vertices()[index].z == 0.0)),
                "only the top is dropped"
            );
        }
        let open = mesh_data::Triangulation::from_vertices_and_faces(vertices, faces[..2].to_vec()).unwrap();
        assert!(floor_and_walls(&open).unwrap().is_none());
    }
}
