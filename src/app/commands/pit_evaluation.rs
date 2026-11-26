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
            evaluation::{BenchFigures, Evaluation, Figures, Measured, RehabFigures, RehabSettings, ShellFigures, Waiting},
        },
        solid_reserves::{ReserveBand, ReservePartition, ReservePiece, ReserveTotals},
        spatial::TriangleBvh,
    },
    ui::state::SolidRegion,
};

/// What a design is measured with, ready for a worker.
struct Inputs {
    design: Design,
    /// The fingerprint of the design, as its object keeps it.
    design_inputs: u64,
    topography: Arc<mesh_data::Triangulation>,
    topography_spatial: Arc<TriangleBvh>,
    model: Option<ModelInputs>,
    /// The optimisation shell it is set against, if one is chosen.
    shell: Option<Arc<mesh_data::Triangulation>>,
    /// How a dump is rehabilitated, if it is.
    rehab: Option<RehabSettings>,
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
    /// What must be loaded first to measure it, if anything.
    missing: Option<String>,
    design: &'a Design,
    design_inputs: u64,
    topography: &'a crate::model::triangulation::OpenTriangulation,
    model: Option<&'a crate::model::block_model::OpenBlockModel>,
    fields: Vec<ReserveField>,
    shell: Option<&'a crate::model::triangulation::OpenTriangulation>,
    rehab: Option<RehabSettings>,
}

impl Plan<'_> {
    /// A worker's copy of it.
    fn inputs(&self) -> Inputs {
        Inputs {
            design: self.design.clone(),
            design_inputs: self.design_inputs,
            topography: self.topography.mesh.clone(),
            topography_spatial: self.topography.spatial.clone(),
            model: self.model.map(|model| ModelInputs {
                data: model.model.clone(),
                blocks: model.blocks.clone(),
                mapping: model.reserve_mapping.clone(),
                fields: self.fields.clone(),
            }),
            shell: self.shell.map(|shell| shell.mesh.clone()),
            rehab: self.rehab.clone(),
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
        // What must be loaded to measure it, but not to know whether what
        // it was measured with has changed: the first not loaded, by name.
        let mut missing: Option<String> = None;
        let mut need = |loaded: bool, name: &str| {
            if !loaded && missing.is_none() {
                missing = Some(name.to_owned());
            }
        };
        need(surface.state.loaded && surface.state.deferred.is_none(), &surface.name);
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
                need(model.state.loaded && resident, &model.name);
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
                need(shell.state.loaded && shell.state.deferred.is_none(), &shell.name);
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
            // The mapping and fields, not the values: a model's values are
            // read only when it is loaded.
            parts.push(serde_json::to_vec(&fields).unwrap_or_default());
            parts.push(serde_json::to_vec(&model.reserve_mapping).unwrap_or_default());
        }
        if let Some(rehab) = settings.rehab.as_ref().filter(|_| settings.kind == crate::model::pit_design::Kind::Dump) {
            parts.push(serde_json::to_vec(rehab).unwrap_or_default());
        }
        let key = fingerprint(&parts.iter().map(Vec::as_slice).collect::<Vec<_>>());
        Ok(Plan {
            key,
            missing,
            design,
            design_inputs: derived.inputs,
            topography: surface,
            model,
            fields,
            shell,
            rehab: settings.rehab.clone().filter(|_| settings.kind == crate::model::pit_design::Kind::Dump),
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
            Ok(Plan { missing: Some(name), .. }) => Some(Waiting::NotLoaded(name)),
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
                let plan = self.pit_evaluation_plan(object).ok().filter(|plan| plan.missing.is_none())?;
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
    let solid = super::triangulation::solid_between::build_solid_mesh(
        &surface,
        &inputs.topography,
        region,
        super::triangulation::solid_between::BuiltFor::Measure,
        &progress.phase(0.0, 0.3),
    )?;
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
    let shell = inputs
        .shell
        .as_ref()
        .map(|shell| match against_shell(&inputs.topography, inputs.model.as_ref(), &surface, shell, cancel, progress) {
            Ok(figures) => Ok(figures),
            Err(error) if error.is::<crate::app::jobs::Cancelled>() => Err(error),
            Err(error) => Ok(ShellFigures {
                problem: Some(format!("{error:#}")),
                ..ShellFigures::default()
            }),
        });
    let shell = shell.transpose()?;
    progress.set_fraction(1.0);
    // A dump reshaped for its rehabilitation, where it asks.
    let rehab = inputs.rehab.as_ref().filter(|_| design.dump).map(|settings| {
        match super::pit_rehab::rehabilitate(
            design,
            &surface,
            &solid.mesh,
            solid.volume,
            settings,
            (&inputs.topography, &inputs.topography_spatial),
            cancel,
        ) {
            Ok(figures) => Ok(figures),
            Err(error) if error.is::<crate::app::jobs::Cancelled>() => Err(error),
            Err(error) => Ok(RehabFigures {
                problem: Some(format!("{error:#}")),
                ..RehabFigures::default()
            }),
        }
    });
    let rehab = rehab.transpose()?;
    let daylight = daylight_lines(&surface, &inputs.topography, &inputs.topography_spatial);
    Ok(Evaluation {
        key: 0,
        problem: None,
        total,
        benches,
        shell,
        rehab,
        daylight,
        design: inputs.design_inputs,
    })
}

/// Where `surface` meets `ground`: each of its triangles' edges the ground
/// crosses, at the point it crosses it, chained into strings. The ground is
/// read at the surface's points, and taken as straight between them across
/// each triangle.
fn daylight_lines(surface: &mesh_data::Triangulation, ground: &mesh_data::Triangulation, spatial: &TriangleBvh) -> Vec<crate::model::pit_design::DesignString> {
    use std::collections::{HashMap, HashSet};

    use glam::DVec3;
    use rayon::prelude::*;
    let bounds = ground.bounds();
    let above = bounds.max.z + ((bounds.max.z - bounds.min.z).abs() * 1.0e-6).max(1.0);
    let vertices: Vec<DVec3> = surface.vertices().iter().map(|vertex| DVec3::new(vertex.x, vertex.y, vertex.z)).collect();
    // How far above the ground each point stands; a point on it, a hair
    // above, so an edge is crossed where it truly goes from one side to the
    // other.
    let height: Vec<Option<f64>> = vertices
        .par_iter()
        .map(|point| {
            let inside = point.x >= bounds.min.x && point.x <= bounds.max.x && point.y >= bounds.min.y && point.y <= bounds.max.y;
            let rl = inside
                .then(|| spatial.ray_hit(ground, point.truncate().extend(above), -DVec3::Z).map(|hit| hit.z))
                .flatten()?;
            let height = point.z - rl;
            Some(if height.abs() < 1.0e-9 { 1.0e-9 } else { height })
        })
        .collect();
    // Each crossed edge is a point of the line, joined to the other crossed
    // edge of each triangle it is in.
    let mut at: HashMap<(u32, u32), DVec3> = HashMap::new();
    let mut joins: HashMap<(u32, u32), Vec<(u32, u32)>> = HashMap::new();
    for face in surface.face_vertex_indices_iter() {
        let corners = face.map(|index| index as u32);
        let Some(heights) = corners.iter().map(|&index| height[index as usize]).collect::<Option<Vec<f64>>>() else {
            continue;
        };
        let mut crossed = Vec::with_capacity(2);
        for side in 0..3 {
            let (a, b) = (side, (side + 1) % 3);
            if (heights[a] > 0.0) == (heights[b] > 0.0) {
                continue;
            }
            let (from, to) = (corners[a], corners[b]);
            let key = (from.min(to), from.max(to));
            let t = heights[a] / (heights[a] - heights[b]);
            at.entry(key).or_insert_with(|| vertices[from as usize].lerp(vertices[to as usize], t));
            crossed.push(key);
        }
        if let [first, second] = crossed[..] {
            joins.entry(first).or_default().push(second);
            joins.entry(second).or_default().push(first);
        }
    }
    // Walked from each end first, then round each loop.
    let mut seen: HashSet<(u32, u32)> = HashSet::new();
    let mut lines = Vec::new();
    let mut starts: Vec<(u32, u32)> = joins.iter().filter(|(_, next)| next.len() == 1).map(|(key, _)| *key).collect();
    starts.sort_unstable();
    let mut rest: Vec<(u32, u32)> = joins.keys().copied().collect();
    rest.sort_unstable();
    for start in starts.into_iter().chain(rest) {
        if !seen.insert(start) {
            continue;
        }
        let mut chain = vec![start];
        let mut current = start;
        while let Some(next) = joins.get(&current).and_then(|next| next.iter().copied().find(|key| !seen.contains(key))) {
            seen.insert(next);
            chain.push(next);
            current = next;
        }
        let closed = chain.len() > 2 && joins.get(&current).is_some_and(|next| next.contains(&start));
        let points: Vec<DVec3> = chain.iter().filter_map(|key| at.get(key).copied()).collect();
        if points.len() >= 2 {
            lines.push(crate::model::pit_design::DesignString {
                kind: crate::model::pit_design::StringKind::Daylight,
                points,
                closed,
            });
        }
    }
    lines
}

/// How far round both the design and the shell the topography is kept to
/// set them into: well clear of either, so the two landforms are the same
/// ground at their edges.
pub(super) const LANDFORM_MARGIN: f64 = 50.0;

/// The faces of `mesh` within `(min, max)` in plan.
pub(super) fn cropped(mesh: &mesh_data::Triangulation, (min, max): (glam::DVec2, glam::DVec2)) -> anyhow::Result<mesh_data::Triangulation> {
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
/// What the design takes beyond the shell and leaves inside it, measured
/// between the two landforms (the topography with each set in) column by
/// column: where the shell's landform stands above the design's, ground the
/// design takes beyond it; where below, ground inside it the design leaves.
///
/// The landforms are single valued, so the ground between them is no more
/// than the gap at each point, read off a raster of both a fraction of a
/// block across; each block holds the share of its height the gap spans,
/// averaged over points across it. A solid between them is no use: they
/// are the same ground beyond both pits, and surfaces lying on each other
/// leave it open, where the block model cannot measure it.
fn against_shell(
    topography: &mesh_data::Triangulation,
    model: Option<&ModelInputs>,
    design: &mesh_data::Triangulation,
    shell: &mesh_data::Triangulation,
    cancel: &crate::app::jobs::CancelFlag,
    progress: &crate::model::progress::Progress,
) -> anyhow::Result<ShellFigures> {
    use rayon::prelude::*;
    let own = floor_and_walls(shell)?;
    let shell = own.as_ref().unwrap_or(shell);
    // The ground round both, set with each in turn.
    let (design_bounds, shell_bounds) = (design.bounds(), shell.bounds());
    let min = glam::DVec2::new(design_bounds.min.x.min(shell_bounds.min.x), design_bounds.min.y.min(shell_bounds.min.y)) - glam::DVec2::splat(LANDFORM_MARGIN);
    let max = glam::DVec2::new(design_bounds.max.x.max(shell_bounds.max.x), design_bounds.max.y.max(shell_bounds.max.y)) + glam::DVec2::splat(LANDFORM_MARGIN);
    let ground = cropped(topography, (min, max))?;
    let spatial = TriangleBvh::build(&ground);
    let include = super::triangulation::include::landform_with;
    let (with_design, with_shell) = rayon::join(
        || include(&ground, &spatial, design, &progress.phase(0.5, 0.6)),
        || include(&ground, &spatial, shell, &progress.phase(0.6, 0.7)),
    );
    let (with_design, with_shell) = (with_design?, with_shell?);
    anyhow::ensure!(!cancel.is_cancelled(), crate::app::jobs::Cancelled);

    // Both landforms on a raster: a quarter of a block across, and never
    // coarser than SHELL_RASTER.
    let block = model.and_then(|model| model.blocks.get(0)).map(|bounds| bounds.upper - bounds.lower);
    let spacing = block.map_or(SHELL_RASTER, |size| (size.x.min(size.y) / 4.0).clamp(SHELL_RASTER / 8.0, SHELL_RASTER));
    let columns = (((max.x - min.x) / spacing).ceil() as usize).max(1);
    let rows = (((max.y - min.y) / spacing).ceil() as usize).max(1);
    let (design_spatial, shell_spatial) = rayon::join(|| TriangleBvh::build(&with_design), || TriangleBvh::build(&with_shell));
    let above = design_bounds.max.z.max(shell_bounds.max.z).max(ground.bounds().max.z) + 1.0;
    let read = |mesh: &mesh_data::Triangulation, spatial: &TriangleBvh, point: glam::DVec2| spatial.ray_hit(mesh, point.extend(above), -glam::DVec3::Z).map(|hit| hit.z);
    let cells: Vec<Option<(f64, f64)>> = (0..rows)
        .into_par_iter()
        .flat_map_iter(|row| {
            let (read, design_spatial, shell_spatial, with_design, with_shell) = (&read, &design_spatial, &shell_spatial, &with_design, &with_shell);
            (0..columns).map(move |column| {
                let point = min + glam::DVec2::new((column as f64 + 0.5) * spacing, (row as f64 + 0.5) * spacing);
                Some((read(with_design, design_spatial, point)?, read(with_shell, shell_spatial, point)?))
            })
        })
        .collect();
    anyhow::ensure!(!cancel.is_cancelled(), crate::app::jobs::Cancelled);
    // The volumes between them, and the band of RL they part over.
    let area = spacing * spacing;
    let (mut gained, mut left, mut low, mut high) = (0.0, 0.0, f64::INFINITY, f64::NEG_INFINITY);
    for &(design_z, shell_z) in cells.iter().flatten() {
        if (design_z - shell_z).abs() > SAME_GROUND {
            gained += (shell_z - design_z).max(0.0) * area;
            left += (design_z - shell_z).max(0.0) * area;
            low = low.min(design_z.min(shell_z));
            high = high.max(design_z.max(shell_z));
        }
    }
    let mut figures = ShellFigures {
        gained: Figures { volume: gained, measured: None },
        left: Figures { volume: left, measured: None },
        problem: None,
    };
    progress.set_fraction(0.8);
    let (Some(model), true) = (model, low < high) else {
        return Ok(figures);
    };
    let cell_at = |point: glam::DVec2| {
        let offset = (point - min) / spacing;
        if offset.x < 0.0 || offset.y < 0.0 {
            return None;
        }
        let (column, row) = (offset.x as usize, offset.y as usize);
        (column < columns && row < rows).then(|| cells[row * columns + column]).flatten()
    };
    let measure = |_: &mut crate::model::solid_reserves::MeasureScratch, index: usize| -> anyhow::Result<crate::model::solid_reserves::BlockOverlaps> {
        let nothing = || crate::model::solid_reserves::BlockOverlaps { volume: 0.0, hits: Vec::new() };
        let Some(bounds) = model.blocks.get(index) else {
            return Ok(nothing());
        };
        let size = bounds.upper - bounds.lower;
        if !size.is_finite() || size.min_element() <= 0.0 {
            return Ok(nothing());
        }
        let centre = model.data.local_to_world(bounds.lower + size * 0.5);
        let (floor, roof) = (centre.z - size.z * 0.5, centre.z + size.z * 0.5);
        let reach = size.x.max(size.y);
        if roof <= low || floor >= high || centre.x < min.x - reach || centre.x > max.x + reach || centre.y < min.y - reach || centre.y > max.y + reach {
            return Ok(nothing());
        }
        // The gap's share of the block's height at points across it.
        let (mut taken, mut kept, mut points) = (0.0, 0.0, 0usize);
        for step in 0..BLOCK_SAMPLES * BLOCK_SAMPLES {
            let (i, j) = (step % BLOCK_SAMPLES, step / BLOCK_SAMPLES);
            let local = bounds.lower + size * glam::DVec3::new((i as f64 + 0.5) / BLOCK_SAMPLES as f64, (j as f64 + 0.5) / BLOCK_SAMPLES as f64, 0.5);
            points += 1;
            let Some((design_z, shell_z)) = cell_at(model.data.local_to_world(local).truncate()) else {
                continue;
            };
            let under = design_z.min(shell_z);
            let spans = |top: f64| (top.min(roof) - under.max(floor)).max(0.0);
            taken += spans(shell_z);
            kept += spans(design_z);
        }
        let share = |depth: f64| depth / (points as f64 * size.z);
        let hits: Vec<(usize, f64)> = [(0, share(taken)), (1, share(kept))].into_iter().filter(|(_, fraction)| *fraction > 0.0).collect();
        Ok(crate::model::solid_reserves::BlockOverlaps {
            volume: size.x * size.y * size.z,
            hits,
        })
    };
    let outcome = crate::model::solid_reserves::compute_by(&model.data, &model.blocks, &model.mapping, &model.fields, 2, false, cancel, progress, Some(&measure))?;
    figures.gained.measured = outcome.totals.first().map(|totals| Measured::of(totals, &model.fields));
    figures.left.measured = outcome.totals.get(1).map(|totals| Measured::of(totals, &model.fields));
    Ok(figures)
}

/// The coarsest a shell comparison's raster is, in metres; finer, at a
/// quarter of a block, against a block model.
const SHELL_RASTER: f64 = 2.5;

/// How many points across a block, each way, the gap between two landforms
/// is read at.
const BLOCK_SAMPLES: usize = 4;

/// How far apart two landforms may stand and still be the same ground.
const SAME_GROUND: f64 = 1.0e-3;

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

    /// A square pit, its floor `half_floor` either side of the centre at
    /// `floor`, its crest `half_crest` either side on the flat ground at 10.
    fn square_pit(half_floor: f64, floor: f64, half_crest: f64) -> mesh_data::Triangulation {
        let ring = |half: f64, z: f64| [(-half, -half), (half, -half), (half, half), (-half, half)].map(|(x, y)| mesh_data::Vertex::new(x, y, z));
        let vertices: Vec<mesh_data::Vertex> = ring(half_floor, floor).into_iter().chain(ring(half_crest, 10.0)).collect();
        let mut faces: Vec<[u32; 3]> = vec![[0, 1, 2], [0, 2, 3]];
        for side in 0..4u32 {
            let (a, b) = (side, (side + 1) % 4);
            faces.push([a, b + 4, b]);
            faces.push([a, a + 4, b + 4]);
        }
        mesh_data::Triangulation::from_vertices_and_faces(vertices, faces).unwrap()
    }

    /// Volume of a square frustum between half widths `a` and `b`, `h` deep.
    fn frustum(a: f64, b: f64, h: f64) -> f64 {
        let (low, high) = ((2.0 * a).powi(2), (2.0 * b).powi(2));
        h / 3.0 * (low + high + (low * high).sqrt())
    }

    /// A design inside its shell leaves the ground between them and gains
    /// none; set the other way round, it gains that ground and leaves none.
    /// The two landforms are the same ground beyond both pits, which a solid
    /// between them could not be closed over.
    #[test]
    fn a_shell_comparison_measures_the_ground_between_the_landforms() {
        let ground = mesh_data::Triangulation::from_vertices_and_faces(
            [(-100.0, -100.0), (100.0, -100.0), (100.0, 100.0), (-100.0, 100.0)]
                .map(|(x, y)| mesh_data::Vertex::new(x, y, 10.0))
                .to_vec(),
            vec![[0, 1, 2], [0, 2, 3]],
        )
        .unwrap();
        let (small, large) = (square_pit(10.0, 0.0, 20.0), square_pit(15.0, -10.0, 30.0));
        let between = frustum(15.0, 30.0, 20.0) - frustum(10.0, 20.0, 10.0);
        let cancel = crate::app::jobs::CancelFlag::default();
        let progress = crate::model::progress::Progress::default();
        let inside = against_shell(&ground, None, &small, &large, &cancel, &progress).unwrap();
        let outside = against_shell(&ground, None, &large, &small, &cancel, &progress).unwrap();
        for (measured, expected) in [
            (inside.gained.volume, 0.0),
            (inside.left.volume, between),
            (outside.gained.volume, between),
            (outside.left.volume, 0.0),
        ] {
            assert!((measured - expected).abs() <= between * 0.01, "{measured} against {expected}");
        }
    }
}
