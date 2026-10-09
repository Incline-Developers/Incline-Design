//! Thickness points and thickness surfaces, the App side: the surface a run
//! measures against, the seam, holes and measured pairs it reads, and the
//! point layer, table, saved values and report it leaves; then the seam's
//! other surface from the latest run.
//!
//! The reference surface is read off its own grid, so a surface from a
//! saved project works as well as one built this session. The seam a
//! surface was built from is remembered for the session only, to prefill
//! the dialog.

use std::sync::Arc;

use anyhow::{Context, Result};

use crate::{
    app::{
        App,
        commands::triangulation::reference_surface::seam::{self, RunProblem, SeamGrid, ThicknessRunRecord, ThicknessSample},
        jobs::CancelFlag,
    },
    i18n::tr,
    model::{
        Command, LayerId, SceneEntityId,
        drill_hole::{DrillHoleId, DrillHoleRef, OpenDrillHoleDataset, ReferenceSide, ReferenceTarget},
        grid_surface::{GridSurface, NotAGrid},
        thickness_points::{self, ColumnClash, Intercept, InterceptSource, LeftOut, LeftOutReason, PairsError, ThicknessRun},
        triangulation::{GeneratedTriangulation, TriangulationId},
    },
    ui::state::{GridCheck, PairsFile, SeamChoice, SeamSurfaceDraft, SeamTable, ThicknessPointsDraft, ThicknessTable, TriSurfaceType},
    userspace_log, userspace_warn,
};

/// The names one seam's results sort together under. For seam A built on
/// its roof: the points "A Roof points", the surface "A Roof", the thickness
/// points "A thickness points" and the made surface "A Floor".
pub(crate) mod seam_names {
    use crate::{i18n::tr, model::drill_hole::ReferenceSide};

    pub(crate) fn points_layer(seam: &str, side: ReferenceSide) -> String {
        tr!("cmd-seam-surface-points-layer", seam = seam.to_owned(), side = side.label())
    }

    pub(crate) fn surface(seam: &str, side: ReferenceSide) -> String {
        tr!("cmd-seam-surface-name", seam = seam.to_owned(), side = side.label())
    }

    pub(crate) fn thickness_layer(seam: &str) -> String {
        tr!("cmd-thickness-points-layer", seam = seam.to_owned())
    }

    /// The side the other surface of a seam lies on.
    pub(crate) fn other(side: ReferenceSide) -> ReferenceSide {
        match side {
            ReferenceSide::Roof => ReferenceSide::Floor,
            ReferenceSide::Floor => ReferenceSide::Roof,
        }
    }
}

/// How many left-out items the report names one by one before it counts
/// the rest.
const LEFT_OUT_LINES: usize = 50;

/// What a reference points layer was made from: the field and working
/// section, and the side picked.
#[derive(Clone, Debug)]
pub(crate) struct ReferenceSource {
    pub(crate) field: String,
    pub(crate) target: ReferenceTarget,
    pub(crate) side: ReferenceSide,
}

/// The seam a run measures: the categorical field, the working section or
/// code, and the side the reference surface passes through.
#[derive(Clone, Debug)]
pub(crate) struct ReferenceSeam {
    pub(crate) field: String,
    pub(crate) target: ReferenceTarget,
    pub(crate) side: ReferenceSide,
}

/// A surface and its geometry as a run reads it, taken on the UI thread so
/// the worker never sees the project.
struct SurfaceRead {
    name: String,
    revision: u64,
    mesh: Arc<crate::model::formats::mesh_data::Triangulation>,
    spatial: Arc<crate::model::spatial::TriangleBvh>,
}

/// Per dataset, each hole holding the section by name, with its true
/// thickness or `None` where it was left out: what a run saves.
type HoleValues = Vec<(DrillHoleId, Vec<(String, Option<f64>)>)>;

/// What a thickness surface is made from, read off the selected surface.
struct SeamInput {
    surface: SurfaceRead,
    output: String,
    run: String,
    samples: Vec<ThicknessSample>,
    side: ReferenceSide,
}

/// One run's result as the App applies it.
struct Made {
    run: ThicknessRun,
    /// Holes that never hold the section.
    without: usize,
    saved: HoleValues,
}

/// The holes of one dataset gathered for a run: their intercepts, and those
/// left out before measuring.
struct DatasetIntercepts {
    id: DrillHoleId,
    intercepts: Vec<Intercept>,
    left_out: Vec<LeftOut>,
}

impl<'a> App<'a> {
    pub(crate) fn remember_reference_source(&mut self, runtime_id: u32, layer: LayerId, source: ReferenceSource) {
        self.session.reference_sources.insert((runtime_id, layer), source);
    }

    /// The seam a surface's points were picked on, when they all sit on one
    /// reference points layer made this session.
    pub(crate) fn reference_source_of(&self, runtime_id: u32, layers: &[LayerId]) -> Option<ReferenceSource> {
        match layers {
            [layer] => self.session.reference_sources.get(&(runtime_id, *layer)).cloned(),
            _ => None,
        }
    }

    /// Remember the seam behind the surface just inserted, which inserting
    /// made the active one, to prefill its thickness points dialog.
    pub(crate) fn keep_surface_source(&mut self, source: Option<ReferenceSource>) {
        if let (Some(id), Some(source)) = (self.active_triangulation, source) {
            self.session.surface_seams.insert(id, source);
        }
    }

    /// Open the dialog on the one surface selected, with any holes selected
    /// beside it, its seam prefilled when the surface was built this
    /// session.
    pub(crate) fn open_thickness_points(&mut self) {
        let Some(surface) = self.one_selected_surface() else {
            return;
        };
        let mut holes = Vec::new();
        self.for_each_reference_hole(|hole| holes.push(hole));
        holes.sort_unstable_by_key(|hole| (hole.dataset.0, hole.hole));
        let seam = seam_prefill(self.session.surface_seams.get(&surface), self.editor.last_seam.as_ref());
        self.editor.thickness_points_dialog = Some(ThicknessPointsDraft {
            surface,
            surface_label: self.triangulation_name(surface),
            holes,
            seam,
            codes: None,
            pairs: None,
            then_surface: true,
            grid: GridCheck::Checking,
        });
        self.check_grid(surface);
    }

    /// Read the surface as a grid in the background, so the open dialog can
    /// say whether it can be measured against before Make is pressed.
    fn check_grid(&mut self, surface: TriangulationId) {
        let Some(read) = self.surface_read(surface) else {
            self.set_grid_check(surface, GridCheck::Refused(tr!("cmd-thickness-points-surface-gone")));
            return;
        };
        let key = crate::app::jobs::JobKey::Triangulation(surface);
        let compute = move |_: &CancelFlag| -> Result<GridCheck> {
            Ok(match GridSurface::read(read.mesh, read.spatial) {
                Ok(_) => GridCheck::Grid,
                Err(problem) => GridCheck::Refused(not_a_grid(&read.name, problem)),
            })
        };
        let apply = move |app: &mut App, result: Result<GridCheck>| {
            app.set_grid_check(surface, result.unwrap_or_else(|error| GridCheck::Refused(format!("{error:#}"))));
        };
        self.spawn_job(tr!("cmd-thickness-points-checking-grid"), vec![key], compute, apply);
    }

    /// Hand a grid check to whichever dialog is open on `surface`.
    fn set_grid_check(&mut self, surface: TriangulationId, check: GridCheck) {
        if let Some(draft) = self.editor.thickness_points_dialog.as_mut().filter(|draft| draft.surface == surface) {
            draft.grid = check.clone();
        }
        if let Some(draft) = self.editor.seam_surface_dialog.as_mut().filter(|draft| draft.surface == surface) {
            draft.grid = check;
        }
    }

    /// The surface's name, revision and geometry, when it is loaded.
    fn surface_read(&self, surface: TriangulationId) -> Option<SurfaceRead> {
        let triangulation = self.triangulations.iter().find(|triangulation| triangulation.id == surface && triangulation.state.loaded)?;
        Some(SurfaceRead {
            name: triangulation.name.clone(),
            revision: triangulation.state.revision(),
            mesh: Arc::clone(&triangulation.mesh),
            spatial: Arc::clone(&triangulation.spatial),
        })
    }

    /// Ask for a measured pairs file; the answer lands in the open dialog.
    pub(crate) fn choose_thickness_pairs(&mut self) {
        let filter = tr!("cmd-thickness-points-pairs-filter");
        #[cfg(not(target_arch = "wasm32"))]
        self.spawn_file_dialog(async move {
            let handle = rfd::AsyncFileDialog::new().add_filter(filter, &["csv"]).pick_file().await?;
            let path = handle.path().to_owned();
            let name = path.file_name().map_or_else(|| path.display().to_string(), |name| name.to_string_lossy().into_owned());
            Some(super::file::FileDialogAction::ThicknessPairs(Ok(PairsFile { name, path })))
        });
        #[cfg(target_arch = "wasm32")]
        self.spawn_file_dialog(async move {
            let handle = rfd::AsyncFileDialog::new().add_filter(filter, &["csv"]).pick_file().await?;
            let file = crate::model::input::read_browser_handle(handle).await.map(|file| PairsFile {
                name: file.source.name,
                bytes: Arc::from(file.bytes),
            });
            Some(super::file::FileDialogAction::ThicknessPairs(file))
        });
    }

    pub(crate) fn set_thickness_pairs(&mut self, file: std::result::Result<PairsFile, String>) {
        match file {
            Ok(file) => match self.editor.thickness_points_dialog.as_mut() {
                Some(draft) => draft.pairs = Some(file),
                None => userspace_warn!("{}", tr!("cmd-thickness-points-dialog-closed")),
            },
            Err(error) => userspace_warn!("{}", tr!("cmd-thickness-points-pairs-unreadable", error = error)),
        }
    }

    /// Measure `seam` against `surface` at every hole holding it among
    /// `holes` (none: every loaded hole), and at each measured pair, into a
    /// new point layer; with `then_surface`, then make the seam's other
    /// surface from that run.
    pub(crate) fn make_thickness_points(
        &mut self,
        surface: TriangulationId,
        holes: Vec<DrillHoleRef>,
        seam: ReferenceSeam,
        pairs: Option<PairsFile>,
        then_surface: bool,
    ) -> Result<()> {
        let project = self.workspace.active_project().with_context(|| tr!("cmd-thickness-points-open-project"))?;
        let runtime_id = project.runtime_id;
        let project_key = crate::app::jobs::JobKey::Project {
            runtime_id,
            document_revision: project.project.document.revision(),
        };
        let read = self.surface_read(surface).with_context(|| tr!("cmd-thickness-points-surface-gone"))?;
        let holes: Vec<DrillHoleRef> = if holes.is_empty() {
            self.drill_holes
                .iter()
                .filter(|dataset| dataset.state.loaded)
                .flat_map(|dataset| (0..dataset.dataset.holes.len()).map(|hole| DrillHoleRef { dataset: dataset.id, hole }))
                .collect()
        } else {
            holes
        };
        // Gathered here: one pass over each hole's intervals, and the worker
        // never sees the datasets.
        let mut involved: Vec<&OpenDrillHoleDataset> = Vec::new();
        for reference in &holes {
            if !involved.iter().any(|dataset| dataset.id == reference.dataset)
                && let Some(dataset) = self.drill_holes.iter().find(|dataset| dataset.id == reference.dataset && dataset.state.loaded)
            {
                involved.push(dataset);
            }
        }
        let (codes_by_dataset, _) = super::drill_hole::reference_codes(&involved, &seam.field, &seam.target);
        let mut groups: Vec<DatasetIntercepts> = Vec::new();
        let mut without = 0usize;
        for reference in &holes {
            let hole = involved
                .iter()
                .find(|dataset| dataset.id == reference.dataset)
                .and_then(|dataset| dataset.dataset.holes.get(reference.hole));
            let Some(hole) = hole else {
                without += 1;
                continue;
            };
            let codes = codes_by_dataset
                .iter()
                .find(|(id, _)| *id == reference.dataset)
                .map_or(&[][..], |(_, codes)| codes.as_slice());
            let Some(found) = thickness_points::hole_intercept(hole, &seam.field, codes) else {
                without += 1;
                continue;
            };
            let group = match groups.iter().position(|group| group.id == reference.dataset) {
                Some(index) => &mut groups[index],
                None => {
                    groups.push(DatasetIntercepts {
                        id: reference.dataset,
                        intercepts: Vec::new(),
                        left_out: Vec::new(),
                    });
                    groups.last_mut().expect("just pushed")
                }
            };
            match found {
                Ok(intercept) => group.intercepts.push(intercept),
                Err(left) => group.left_out.push(left),
            }
        }

        // Measured a dataset at a time, so each hole's value is found by its
        // name within its own dataset when it is saved.
        let (mesh, spatial, revision, surface_name) = (Arc::clone(&read.mesh), Arc::clone(&read.spatial), read.revision, read.name.clone());
        let compute = move |cancel: &CancelFlag| -> Result<Made> {
            let fit = GridSurface::read(mesh, spatial).map_err(|problem| anyhow::anyhow!("{}", not_a_grid(&surface_name, problem)))?;
            let mut run = ThicknessRun::default();
            let mut saved = Vec::new();
            for group in groups {
                let mut measured = thickness_points::thickness_points(group.intercepts, &fit, cancel)?;
                measured.left_out.splice(0..0, group.left_out);
                let mut values: Vec<(String, Option<f64>)> = Vec::new();
                values.extend(
                    measured
                        .points
                        .iter()
                        .filter_map(|point| hole_name(&point.source).map(|name| (name, Some(point.true_thickness)))),
                );
                values.extend(measured.left_out.iter().filter_map(|left| hole_name(&left.source).map(|name| (name, None))));
                saved.push((group.id, values));
                run.points.append(&mut measured.points);
                run.left_out.append(&mut measured.left_out);
            }
            if let Some(file) = pairs {
                let bytes = pairs_bytes(&file)?;
                let (pairs, mut rows_left) = thickness_points::read_pairs(&bytes).map_err(|error| anyhow::anyhow!("{}", pairs_error(&file.name, error)))?;
                let mut measured = thickness_points::thickness_points(pairs, &fit, cancel)?;
                measured.left_out.append(&mut rows_left);
                measured.left_out.sort_by_key(|left| match left.source {
                    InterceptSource::Hole { .. } => 0,
                    InterceptSource::Measured { line, .. } => line,
                });
                run.points.append(&mut measured.points);
                run.left_out.append(&mut measured.left_out);
            }
            Ok(Made { run, without, saved })
        };
        let against = (surface, revision);
        let apply = move |app: &mut App, result: Result<Made>| match result {
            Ok(made) => {
                if app.finish_thickness_points(runtime_id, made, against, read.name, seam) && then_surface {
                    // Only after a run that applied: the surface job reads it.
                    if let Err(error) = app.make_seam_surface(against.0) {
                        userspace_warn!("{}", format!("{error:#}"));
                    }
                }
            }
            Err(error) => crate::userspace_error!("{}", tr!("cmd-thickness-points-failed", error = format!("{error:#}"))),
        };
        self.spawn_job(tr!("cmd-thickness-points-making"), vec![project_key], compute, apply);
        Ok(())
    }

    /// Report a run, lay its points down as a new layer at their roofs with
    /// their table kept beside it, and save each hole's value to its dataset,
    /// all as one undo step. The points are kept as the surface's latest run,
    /// with the revision they were measured against, for its thickness grid.
    /// Whether the run made any points.
    fn finish_thickness_points(&mut self, runtime_id: u32, made: Made, against: (TriangulationId, u64), surface: String, chosen: ReferenceSeam) -> bool {
        let Made { run, without, saved } = made;
        let (seam, field, target) = (chosen.target.name(), chosen.field.as_str(), &chosen.target);
        if self.workspace.active_project().is_none_or(|project| project.runtime_id != runtime_id) {
            userspace_warn!("{}", tr!("cmd-thickness-points-project-changed"));
            return false;
        }
        let measured = run.points.iter().filter(|point| matches!(point.source, InterceptSource::Measured { .. })).count();
        let (mut commands, saves) = self.thickness_saves(&saved, field, target);
        let mut name = String::new();
        let mut made_layer = None;
        if !run.points.is_empty()
            && let Some(project) = self.workspace.active_project_mut()
        {
            let document = &mut project.project.document;
            name = crate::model::project::unique_item_name(seam_names::thickness_layer(seam), document.layers().iter().map(|layer| layer.name.as_str()));
            let layer_id = document.allocate_layer_id();
            made_layer = Some(layer_id);
            let layer = crate::model::Layer {
                id: layer_id,
                name: name.clone(),
                color_index: None,
                color: [1.0, 1.0, 1.0, 1.0],
                loaded: true,
                hidden: false,
                elevation: 0.0,
                folder: None,
                section: crate::model::SectionKind::Modelling,
            };
            let objects: Vec<crate::model::Object> = run
                .points
                .iter()
                .map(|point| crate::model::Object::Point {
                    id: document.allocate_object_id(),
                    layer: layer_id,
                    pos: point.roof,
                    color: crate::model::ObjectColor::ByLayer,
                })
                .collect();
            commands.insert(0, Command::AddLayerSnapshot { layer, objects });
        }
        if !commands.is_empty() {
            self.execute_edit(Command::Batch(commands));
            self.invalidate_geometry();
        }
        userspace_log!(
            "{}",
            tr!(
                "cmd-thickness-points-made",
                name = if name.is_empty() { tr!("cmd-thickness-points-no-layer") } else { name.clone() },
                holes = (run.points.len() - measured).to_string(),
                measured = measured.to_string(),
                left_out = run.left_out.len().to_string(),
                without = without.to_string(),
                surface = surface.clone()
            )
        );
        if !run.left_out.is_empty() {
            userspace_warn!("{}", left_out_report(&run.left_out));
        }
        let repeated: Vec<&str> = run
            .points
            .iter()
            .filter(|point| point.repeated)
            .filter_map(|point| match &point.source {
                InterceptSource::Hole { dhid, .. } => Some(dhid.as_str()),
                InterceptSource::Measured { .. } => None,
            })
            .collect();
        if !repeated.is_empty() {
            userspace_warn!("{}", tr!("cmd-drill-hole-uppermost-run-used-flagged-holes", holes = repeated.join(", ")));
        }
        for line in saves {
            match line {
                Ok(line) => userspace_log!("{}", line),
                Err(line) => userspace_warn!("{}", line),
            }
        }
        self.session.thickness_runs.insert(against.0, run_record(against.1, name.clone(), &chosen, &run.points));
        // The surface stays the selection, so Thickness Surfaces is ready.
        self.select_only([SceneEntityId::Triangulation(against.0)]);
        let Some(layer) = made_layer else {
            return false;
        };
        // Kept rather than shown: the layer's explorer menu shows it.
        let table = Arc::new(ThicknessTable {
            name,
            surface,
            points: run.points,
        });
        self.session.thickness_tables.insert((runtime_id, layer), table);
        true
    }

    /// The edits saving a run's hole values to each dataset's true thickness
    /// column, and a report line per dataset: a refusal where the dataset
    /// holds a column of that name it came in with.
    fn thickness_saves(&self, saved: &HoleValues, field: &str, target: &ReferenceTarget) -> (Vec<Command>, Vec<std::result::Result<String, String>>) {
        let involved: Vec<&OpenDrillHoleDataset> = saved
            .iter()
            .filter_map(|(id, _)| self.drill_holes.iter().find(|dataset| dataset.id == *id && dataset.state.loaded))
            .collect();
        let (codes_by_dataset, _) = super::drill_hole::reference_codes(&involved, field, target);
        let column = thickness_points::TRUE_THICKNESS_COLUMN.to_owned();
        let mut commands = Vec::new();
        let mut lines = Vec::new();
        for dataset in involved {
            let Some((_, values)) = saved.iter().find(|(id, _)| *id == dataset.id) else {
                continue;
            };
            let codes = codes_by_dataset.iter().find(|(id, _)| *id == dataset.id).map_or(&[][..], |(_, codes)| codes.as_slice());
            let holes: Vec<(usize, Option<f64>)> = values
                .iter()
                .filter_map(|(name, value)| dataset.dataset.holes.iter().position(|hole| &hole.dhid == name).map(|index| (index, *value)))
                .collect();
            let name = dataset.name.clone();
            match thickness_points::thickness_column_edit(dataset.id, &dataset.dataset, field, codes, &holes) {
                Ok(Some((command, summary))) => {
                    commands.push(command);
                    let mut line = tr!("cmd-thickness-points-saved", count = summary.written.to_string(), column = column.clone(), dataset = name);
                    if summary.replaced > 0 {
                        line.push('\n');
                        line.push_str(&tr!("cmd-thickness-points-saved-replaced", count = summary.replaced.to_string()));
                    }
                    if summary.cleared > 0 {
                        line.push('\n');
                        line.push_str(&tr!("cmd-thickness-points-saved-cleared", count = summary.cleared.to_string()));
                    }
                    lines.push(Ok(line));
                }
                Ok(None) => lines.push(Ok(tr!("cmd-thickness-points-saved-unchanged", column = column.clone(), dataset = name))),
                Err(ColumnClash) => lines.push(Err(tr!("cmd-thickness-points-column-clash", column = column.clone(), dataset = name))),
            }
        }
        (commands, lines)
    }

    /// The surface as read, the name of the surface it makes and the
    /// thickness run to grid, or the reason there is none.
    fn seam_surface_input(&self, surface: TriangulationId) -> std::result::Result<SeamInput, String> {
        let read = self.surface_read(surface).ok_or_else(|| tr!("cmd-thickness-points-surface-gone"))?;
        let run = seam::current_run(read.revision, self.session.thickness_runs.get(&surface)).map_err(|problem| match problem {
            RunProblem::Missing => tr!("cmd-seam-surface-no-run", name = read.name.clone()),
            RunProblem::Stale => tr!("cmd-seam-surface-stale-run", name = read.name.clone()),
        })?;
        Ok(SeamInput {
            output: seam_names::surface(&run.seam, seam_names::other(run.side)),
            run: run.name.clone(),
            samples: run.samples.clone(),
            side: run.side,
            surface: read,
        })
    }

    /// Open the thickness surfaces dialog on the one surface selected, or
    /// say why it cannot make one.
    pub(crate) fn open_seam_surface(&mut self) {
        let Some(surface) = self.one_selected_surface() else {
            return;
        };
        match self.seam_surface_input(surface) {
            Ok(input) => {
                self.editor.seam_surface_dialog = Some(SeamSurfaceDraft {
                    surface,
                    surface_label: input.surface.name,
                    run_label: tr!("cmd-seam-surface-run", name = input.run, count = input.samples.len().to_string()),
                    output_label: input.output,
                    grid: GridCheck::Checking,
                });
                self.check_grid(surface);
            }
            Err(reason) => userspace_warn!("{}", reason),
        }
    }

    /// Grid the seam's true thickness on the reference surface's lattice and
    /// hang the seam's other surface from it, as a new surface.
    pub(crate) fn make_seam_surface(&mut self, surface: TriangulationId) -> Result<()> {
        let project = self.workspace.active_project().with_context(|| tr!("cmd-thickness-points-open-project"))?;
        let project_key = crate::app::jobs::JobKey::Project {
            runtime_id: project.runtime_id,
            document_revision: project.project.document.revision(),
        };
        let input = self.seam_surface_input(surface).map_err(|reason| anyhow::anyhow!("{reason}"))?;
        let SeamInput {
            surface: read,
            output,
            run,
            samples,
            side,
        } = input;
        let surface_name = read.name.clone();
        let section = crate::model::SectionKind::derived_for(crate::model::MemberKind::Triangulation, [crate::model::SectionKind::Modelling]);
        let name = output.clone();
        let compute = move |cancel: &CancelFlag, progress: &crate::model::progress::Progress| -> Result<(GeneratedTriangulation, SeamGrid, f64)> {
            let fit = GridSurface::read(read.mesh, read.spatial).map_err(|problem| anyhow::anyhow!("{}", not_a_grid(&read.name, problem)))?;
            let mut grid = seam::seam_grid(&fit, side, &samples, cancel, progress)?;
            let spacing = fit.spacing();
            let (vertices, faces) = (std::mem::take(&mut grid.vertices), std::mem::take(&mut grid.faces));
            let generated =
                super::triangulation::session::build_generated_triangulation(name, vertices, faces, TriSurfaceType::Surface, crate::model::triangulation::unique_edges)?;
            Ok((generated, grid, spacing))
        };
        let apply = move |app: &mut App, result: Result<(GeneratedTriangulation, SeamGrid, f64)>| match result {
            Ok((generated, grid, spacing)) => {
                app.insert_generated_triangulation_in(generated, section);
                let made = app.active_triangulation.map(|id| app.triangulation_name(id)).unwrap_or(output);
                userspace_log!(
                    "{}",
                    tr!(
                        "cmd-seam-surface-made",
                        name = made.clone(),
                        nodes = grid.nodes.len().to_string(),
                        spacing = spacing.to_string(),
                        used = grid.used.to_string(),
                        merged = grid.merged.to_string(),
                        held = grid.held.to_string(),
                        surface = surface_name.clone(),
                        run = run
                    )
                );
                if grid.beyond > 0 {
                    userspace_log!(
                        "{}",
                        tr!("cmd-seam-surface-held-edge", count = grid.beyond.to_string(), reach = seam::TREND_REACH.to_string())
                    );
                }
                // Kept rather than shown: the surface's explorer menu shows it.
                if let Some(id) = app.active_triangulation {
                    let table = Arc::new(SeamTable {
                        name: made,
                        surface: surface_name,
                        nodes: grid.nodes,
                    });
                    app.session.seam_tables.insert(id, table);
                }
            }
            Err(error) => crate::userspace_error!("{}", tr!("cmd-seam-surface-failed", error = format!("{error:#}"))),
        };
        self.spawn_job_reporting_progress(tr!("cmd-seam-surface-making"), vec![project_key], compute, apply);
        Ok(())
    }

    /// The one loaded surface selected, or `None` with the count said.
    fn one_selected_surface(&self) -> Option<TriangulationId> {
        let selected: Vec<TriangulationId> = self
            .triangulations
            .iter()
            .filter(|triangulation| triangulation.state.loaded && self.editor.selected_handles.contains(&SceneEntityId::Triangulation(triangulation.id)))
            .map(|triangulation| triangulation.id)
            .collect();
        match selected[..] {
            [surface] => Some(surface),
            _ => {
                userspace_warn!("{}", tr!("cmd-thickness-points-select-one-surface", count = selected.len().to_string()));
                None
            }
        }
    }

    pub(super) fn triangulation_name(&self, id: TriangulationId) -> String {
        self.triangulations
            .iter()
            .find(|triangulation| triangulation.id == id)
            .map(|triangulation| triangulation.name.clone())
            .unwrap_or_default()
    }
}

/// The seam a Thickness Points dialog opens on: the one the surface was
/// built from this session, else the one last chosen, else none yet.
fn seam_prefill(surface: Option<&ReferenceSource>, last: Option<&SeamChoice>) -> SeamChoice {
    match (surface, last) {
        (Some(source), _) => SeamChoice {
            field: Some(source.field.clone()),
            value: Some(source.target.clone()),
            side: source.side,
        },
        (None, Some(last)) => last.clone(),
        (None, None) => SeamChoice::default(),
    }
}

/// A run as its thickness grid takes it: every point's place and true
/// thickness, the seam, and the side the reference surface passes through,
/// which decides whether the grid hangs a floor or stacks a roof.
fn run_record(revision: u64, name: String, chosen: &ReferenceSeam, points: &[thickness_points::ThicknessPoint]) -> ThicknessRunRecord {
    let samples = points
        .iter()
        .map(|point| ThicknessSample {
            name: match &point.source {
                InterceptSource::Hole { dhid, .. } => dhid.clone(),
                InterceptSource::Measured { id, .. } => id.clone(),
            },
            at: point.roof.truncate(),
            thickness: point.true_thickness,
        })
        .collect();
    ThicknessRunRecord {
        revision,
        name,
        seam: chosen.target.name().to_owned(),
        side: chosen.side,
        samples,
    }
}

/// The hole a point or left-out item came from, by name.
fn hole_name(source: &InterceptSource) -> Option<String> {
    match source {
        InterceptSource::Hole { dhid, .. } => Some(dhid.clone()),
        InterceptSource::Measured { .. } => None,
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn pairs_bytes(file: &PairsFile) -> Result<Vec<u8>> {
    std::fs::read(&file.path).with_context(|| tr!("cmd-thickness-points-pairs-not-read", name = file.name.clone()))
}

#[cfg(target_arch = "wasm32")]
fn pairs_bytes(file: &PairsFile) -> Result<Arc<[u8]>> {
    Ok(Arc::clone(&file.bytes))
}

/// Why `name` cannot be read as one regular grid, in words.
pub(super) fn not_a_grid(name: &str, problem: NotAGrid) -> String {
    let reason = match problem {
        NotAGrid::NoCells => tr!("cmd-thickness-not-a-grid-cells"),
        NotAGrid::TwoHeights => tr!("cmd-thickness-not-a-grid-heights"),
        NotAGrid::TooLarge => tr!("cmd-thickness-not-a-grid-large", budget = crate::model::rbf::NODE_BUDGET.to_string()),
    };
    tr!("cmd-thickness-not-a-grid", name = name.to_owned(), reason = reason)
}

fn pairs_error(name: &str, error: PairsError) -> String {
    match error {
        PairsError::Unreadable(error) => tr!("cmd-thickness-points-pairs-not-csv", name = name.to_owned(), error = error),
        PairsError::MissingColumns(columns) => tr!(
            "cmd-thickness-points-pairs-missing-columns",
            name = name.to_owned(),
            columns = columns.join(", "),
            expected = thickness_points::PAIR_COLUMNS.join(", ")
        ),
    }
}

/// Every hole and measured pair left out, one line each with its reason.
fn left_out_report(left_out: &[LeftOut]) -> String {
    let mut lines = vec![tr!("cmd-thickness-points-left-out-heading", count = left_out.len().to_string())];
    lines.extend(left_out.iter().take(LEFT_OUT_LINES).map(|left| {
        let reason = match left.reason {
            LeftOutReason::NoFloor => tr!("cmd-thickness-points-reason-no-floor"),
            LeftOutReason::Overturned => tr!("cmd-thickness-points-reason-overturned"),
            LeftOutReason::OutsideSurface => tr!("cmd-thickness-points-reason-outside"),
            LeftOutReason::MissingValue => tr!("cmd-thickness-points-reason-missing-value"),
            LeftOutReason::NoTrace => tr!("cmd-thickness-points-reason-no-trace"),
        };
        match &left.source {
            InterceptSource::Hole { dhid, .. } => tr!("cmd-thickness-points-left-out-hole", hole = dhid.clone(), reason = reason),
            InterceptSource::Measured { id, line } => tr!("cmd-thickness-points-left-out-measured", id = id.clone(), line = line.to_string(), reason = reason),
        }
    }));
    if left_out.len() > LEFT_OUT_LINES {
        lines.push(tr!("cmd-thickness-points-and-more", more = (left_out.len() - LEFT_OUT_LINES).to_string()));
    }
    lines.join("\n")
}
