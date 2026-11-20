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
            evaluation::{BenchFigures, Evaluation, Figures, Measured, Waiting},
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
}

/// The block model a design is measured in, and how.
struct ModelInputs {
    data: BlockModelData,
    blocks: Arc<BlockBoundsSource>,
    mapping: Vec<ReserveFieldMapping>,
    fields: Vec<ReserveField>,
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
    fn pit_evaluation_inputs(&self, object: &Object) -> Result<(u64, Inputs), Waiting> {
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
                Some(ModelInputs {
                    data: model.model.clone(),
                    blocks: model.blocks.clone(),
                    mapping: model.reserve_mapping.clone(),
                    fields: fields.clone(),
                })
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
        if let Some(model) = &model {
            parts.push(settings.evaluation.block_model.map_or(0, |id| id.0).to_le_bytes().to_vec());
            parts.push((model.data.metadata.n_blocks as u64).to_le_bytes().to_vec());
            parts.push(serde_json::to_vec(&model.fields).unwrap_or_default());
            parts.push(serde_json::to_vec(&model.mapping).unwrap_or_default());
        }
        let key = fingerprint(&parts.iter().map(Vec::as_slice).collect::<Vec<_>>());
        Ok((
            key,
            Inputs {
                design: design.clone(),
                topography: surface.mesh.clone(),
                model,
            },
        ))
    }

    /// Why the design open in Pit Design is not measured as it stands, or
    /// `None` where its measure is current.
    pub(crate) fn pit_evaluation_waiting(&self, id: ObjectId) -> Option<Waiting> {
        let object = self.operand_object(id)?;
        let (_, pit) = pit_parts(object)?;
        match self.pit_evaluation_inputs(object) {
            Err(waiting) => Some(waiting),
            Ok((key, _)) if pit.evaluation.as_ref().is_some_and(|evaluation| evaluation.key == key) => None,
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
                let (key, inputs) = self.pit_evaluation_inputs(object).ok()?;
                let current = pit.evaluation.as_ref().is_some_and(|evaluation| evaluation.key == key);
                (!current && self.editor.pit_evaluations.get(&object.id()) != Some(&key)).then_some((object.id(), key, inputs))
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
                    let now = app.operand_object(id).and_then(|object| app.pit_evaluation_inputs(object).ok()).map(|(now, _)| now);
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
    progress.set_fraction(1.0);
    Ok(Evaluation {
        key: 0,
        problem: None,
        total,
        benches,
    })
}
