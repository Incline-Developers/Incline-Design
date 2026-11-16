//! Water runs: a storm over a surface, each in a tab of its own.
//!
//! Simulate Water starts a run as a background job and opens its tab, in a
//! row under the viewport bar of the workspace it was started from. The
//! tab's viewport draws the run's own copy of the surface with the water on
//! it at the instant its timeline is at, in bands of depth. A run belongs to
//! its tab: closing the tab stops it and discards it, and nothing of it is
//! saved with the project but the surfaces Save Water Surface adds.

use std::sync::Arc;

use anyhow::Result;

use crate::{
    app::{
        App,
        jobs::{Cancelled, JobKey},
    },
    i18n::tr,
    model::{
        formats::mesh_data,
        progress::Progress,
        triangulation::{OpenTriangulation, TriangulationId},
        water::{BAND_DEPTHS, Ground, WaterError, WaterRun, WaterSettings},
    },
    ui::state::{WaterInstant, WaterRunStatus, WaterShown, WaterTab, Workspace},
};

/// The run's ground, and one id below it for each band of water.
const WATER_SURFACE_ID: u64 = u64::MAX - 2_000_000;
/// The bands' colours, shallowest first.
const BAND_COLORS: [[f32; 4]; BAND_DEPTHS.len()] = [
    [0.55, 0.80, 0.95, 0.55],
    [0.30, 0.65, 0.92, 0.70],
    [0.14, 0.47, 0.84, 0.80],
    [0.07, 0.30, 0.68, 0.88],
    [0.03, 0.16, 0.45, 0.94],
];

/// The band of [`BAND_DEPTHS`] `index`'s colour, for the panel's legend.
pub(crate) fn band_color(index: usize) -> [f32; 4] {
    BAND_COLORS[index.min(BAND_COLORS.len() - 1)]
}

enum RunState {
    Running,
    Failed(String),
    Ready(Arc<WaterRun>),
}

struct RunEntry {
    id: u64,
    title: String,
    /// The workspace the run was started from, whose tab row carries it.
    workspace: Workspace,
    surface: String,
    settings: WaterSettings,
    progress: Option<Progress>,
    state: RunState,
    /// The surface as drawn under the water: the source's own mesh until the
    /// run lands, then the mesh it flowed over.
    ground: OpenTriangulation,
}

/// What the viewport is drawing for the open run: which run, which of its
/// kept instants (`None` for the deepest water of the whole run), and
/// whether the run had landed.
type SceneKey = (u64, Option<usize>, bool);

#[derive(Default)]
pub(crate) struct WaterRuns {
    runs: Vec<RunEntry>,
    next_id: u64,
    /// The open run's ground and water, as the viewport draws them.
    scene: Vec<OpenTriangulation>,
    scene_key: Option<SceneKey>,
}

impl WaterRuns {
    pub(crate) fn scene(&self) -> &[OpenTriangulation] {
        &self.scene
    }

    pub(crate) fn has(&self, id: u64) -> bool {
        self.runs.iter().any(|run| run.id == id)
    }
}

struct Finished {
    run: WaterRun,
    mesh: Arc<mesh_data::Triangulation>,
    face_order: Arc<Vec<u32>>,
}

fn water_error(error: WaterError) -> anyhow::Error {
    match error {
        WaterError::Cancelled => anyhow::Error::new(Cancelled),
        WaterError::Empty => anyhow::anyhow!(tr!("water-error-empty")),
    }
}

/// A run's surface, wrapped for the renderer. Not a project item.
fn scene_surface(id: u64, name: &str, mesh: Arc<mesh_data::Triangulation>, face_order: Arc<Vec<u32>>, color: [f32; 4], line_color: [f32; 4]) -> OpenTriangulation {
    let spatial = Arc::new(crate::model::spatial::TriangleBvh::deferred(mesh.clone()));
    let mut surface = crate::app::commands::solids::preview_triangulation(name.to_owned(), mesh, spatial, Vec::new(), face_order, color, line_color);
    surface.id = TriangulationId(id);
    surface
}

/// A number as a title or a label writes it: no trailing zeros.
pub(crate) fn plain_number(value: f64) -> String {
    let text = format!("{value:.2}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

impl App<'_> {
    /// Start a storm over `surface` and open its tab.
    pub(crate) fn run_water_simulation(&mut self, surface: TriangulationId, settings: WaterSettings) -> Result<()> {
        anyhow::ensure!(settings.is_valid(), tr!("water-error-settings"));
        let Some(source) = self.triangulations.iter().find(|item| item.id == surface && item.state.loaded) else {
            anyhow::bail!(tr!("water-error-no-surface"));
        };
        let id = self.water.next_id;
        self.water.next_id += 1;
        let name = source.name.clone();
        let mesh = source.mesh.clone();
        let ground = scene_surface(WATER_SURFACE_ID, &name, mesh.clone(), source.surface_face_order.clone(), source.color, source.line_color);
        let title = tr!(
            "water-tab-title",
            surface = name.clone(),
            rain = plain_number(settings.rain_mm),
            hours = plain_number(settings.rain_hours)
        );

        let compute = move |cancel: &crate::app::jobs::CancelFlag, progress: &Progress| -> Result<Finished> {
            let signal = cancel.signal();
            let mut ground = Ground::from_mesh(&mesh);
            if let Some(longest) = settings.subdivide_m {
                ground = ground.subdivided(longest, &signal).map_err(water_error)?;
            }
            let run = crate::model::water::simulate(ground, &settings, &signal, progress).map_err(water_error)?;
            let mesh = Arc::new(run.ground.to_mesh().ok_or_else(|| water_error(WaterError::Empty))?);
            let face_order = Arc::new(crate::model::triangulation::spatial_surface_face_order(&mesh));
            Ok(Finished { run, mesh, face_order })
        };
        let progress = self.spawn_job_sharing_progress(
            tr!("water-job", surface = name.clone()),
            vec![JobKey::WaterRun(id)],
            compute,
            move |app, result: Result<Finished>| {
                let Some(entry) = app.water.runs.iter_mut().find(|entry| entry.id == id) else {
                    return;
                };
                entry.progress = None;
                match result {
                    Ok(finished) => {
                        entry.ground = scene_surface(
                            WATER_SURFACE_ID,
                            &entry.surface,
                            finished.mesh,
                            finished.face_order,
                            entry.ground.color,
                            entry.ground.line_color,
                        );
                        let last = finished.run.frames.last();
                        crate::userspace_log!(
                            "{}",
                            tr!(
                                "water-run-done",
                                title = entry.title.clone(),
                                vertices = finished.run.ground.vertices.len().to_string(),
                                volume = format!("{:.0}", last.map_or(0.0, |frame| frame.on_surface))
                            )
                        );
                        entry.state = RunState::Ready(Arc::new(finished.run));
                    }
                    Err(error) if crate::app::jobs::is_cancelled(&error) => {}
                    Err(error) => {
                        crate::userspace_error!("{}", tr!("water-run-failed", title = entry.title.clone(), error = format!("{error:#}")));
                        entry.state = RunState::Failed(format!("{error:#}"));
                    }
                }
                app.redraw_requested = true;
            },
        );
        self.water.runs.push(RunEntry {
            id,
            title,
            workspace: self.editor.active_workspace,
            surface: name,
            settings,
            progress: Some(progress),
            state: RunState::Running,
            ground,
        });
        self.editor.water_dialog.open = false;
        self.open_water_run(id);
        Ok(())
    }

    /// Open run `id`'s tab, putting down whatever the workspace left had
    /// picked up.
    fn open_water_run(&mut self, id: u64) {
        self.editor.clear_scene_selection();
        self.active_triangulation = None;
        self.set_active_tool_from_toolbar(crate::ui::state::ActiveTool::None);
        self.editor.water.open = Some(id);
        self.editor.water.time_s = 0.0;
        self.editor.water.playing = false;
        self.invalidate_geometry();
    }

    /// Close run `id`'s tab: stop it if it is still running, and discard it.
    pub(crate) fn close_water_run(&mut self, id: u64) {
        self.cancel_jobs(|key| *key == JobKey::WaterRun(id));
        let Some(index) = self.water.runs.iter().position(|run| run.id == id) else {
            return;
        };
        let closed = self.water.runs.remove(index);
        if self.editor.water.open == Some(id) {
            // The tab beside it in its row, as a browser does; the last one
            // closed hands the viewport back to the workspace.
            let beside = |run: &&RunEntry| run.workspace == closed.workspace;
            let next = self.water.runs[index..].iter().find(beside).or_else(|| self.water.runs[..index].iter().rev().find(beside));
            self.editor.water.open = next.map(|run| run.id);
            self.editor.water.playing = false;
        }
        self.redraw_requested = true;
    }

    /// Each frame: mirror the runs' tabs into the editor, and keep the open
    /// run's scene and read-out at the instant its timeline shows.
    pub(crate) fn sync_water(&mut self) {
        if self.editor.water.tabs.len() != self.water.runs.len() || self.editor.water.tabs.iter().zip(&self.water.runs).any(|(tab, run)| tab.id != run.id) {
            self.editor.water.tabs = self
                .water
                .runs
                .iter()
                .map(|run| WaterTab {
                    id: run.id,
                    title: run.title.clone(),
                    workspace: run.workspace,
                })
                .collect();
        }
        if self.editor.water.open.is_some_and(|id| !self.water.has(id)) {
            self.editor.water.open = None;
        }
        // Shown only in the workspace whose row carries it.
        let workspace = self.editor.active_workspace;
        let Some(entry) = self
            .editor
            .water
            .open
            .and_then(|id| self.water.runs.iter().find(|run| run.id == id && run.workspace == workspace))
        else {
            self.editor.water.shown = None;
            self.editor.water.playing = false;
            if self.water.scene_key.take().is_some() {
                self.water.scene = Vec::new();
                self.invalidate_geometry();
            }
            return;
        };

        let run = match &entry.state {
            RunState::Ready(run) => Some(run.clone()),
            _ => None,
        };
        let duration = run.as_ref().and_then(|run| run.frames.last()).map_or(0.0, |frame| frame.time_s);
        self.editor.water.time_s = self.editor.water.time_s.clamp(0.0, duration);
        let frame = run
            .as_ref()
            .map(|run| run.frames.partition_point(|frame| frame.time_s <= self.editor.water.time_s).saturating_sub(1));
        let key: SceneKey = (entry.id, frame.filter(|_| !self.editor.water.show_deepest), run.is_some());
        let status = match &entry.state {
            RunState::Running => WaterRunStatus::Running(entry.progress.as_ref().and_then(|progress| progress.snapshot()).map(|snapshot| snapshot.fraction)),
            RunState::Failed(error) => WaterRunStatus::Failed(error.clone()),
            RunState::Ready(_) => WaterRunStatus::Ready,
        };

        let moved = self.water.scene_key != Some(key);
        if moved || self.editor.water.shown.as_ref().is_none_or(|shown| shown.id != entry.id) {
            self.editor.water.shown = Some(WaterShown {
                id: entry.id,
                surface: entry.surface.clone(),
                settings: entry.settings,
                status: status.clone(),
                vertices: run.as_ref().map_or(entry.ground.mesh.vertex_count(), |run| run.ground.vertices.len()),
                faces: run.as_ref().map_or(entry.ground.mesh.face_count(), |run| run.ground.faces.len()),
                area: run.as_ref().map_or(0.0, |run| run.area),
                deepest: run.as_ref().map_or(0.0, |run| run.max_depth.iter().copied().fold(0.0, f32::max)),
                instant: run.as_ref().zip(frame).and_then(|(run, frame)| run.frames.get(frame)).map(|frame| WaterInstant {
                    time_s: frame.time_s,
                    on_surface: frame.on_surface,
                    rained: frame.rained,
                    infiltrated: frame.infiltrated,
                    left: frame.left,
                    ponds: frame.ponds.clone(),
                }),
            });
        } else if let Some(shown) = self.editor.water.shown.as_mut() {
            shown.status = status;
        }
        if !moved {
            return;
        }

        let mut scene = vec![entry.ground.clone()];
        if let Some(run) = &run {
            let depth = match key.1 {
                Some(frame) => &run.frames[frame].depth,
                None => &run.max_depth,
            };
            scene.extend(water_bands(run, depth));
        }
        self.water.scene = scene;
        self.water.scene_key = Some(key);
        self.invalidate_geometry();
    }

    /// Add the water run `id`'s tab is showing to the project, as a surface
    /// at the water's own level.
    pub(crate) fn save_water_surface(&mut self, id: u64) -> Result<()> {
        let Some(entry) = self.water.runs.iter().find(|run| run.id == id) else {
            return Ok(());
        };
        let RunState::Ready(run) = &entry.state else {
            anyhow::bail!(tr!("water-error-not-ready"));
        };
        let frame = run.frames.partition_point(|frame| frame.time_s <= self.editor.water.time_s).saturating_sub(1);
        let (depth, when) = if self.editor.water.show_deepest {
            (&run.max_depth, tr!("water-deepest"))
        } else {
            (&run.frames[frame].depth, crate::ui::elements::water::clock(run.frames[frame].time_s))
        };
        // One surface, with the corners the loose triangles share made one.
        let mut welded = std::collections::HashMap::new();
        let mut vertices = Vec::new();
        let mut faces = Vec::new();
        for band in crate::model::water::water_surface(&run.ground, depth, 0.0) {
            for triangle in band.as_chunks::<3>().0 {
                faces.push(triangle.map(|point| {
                    *welded.entry(point.map(f64::to_bits)).or_insert_with(|| {
                        vertices.push(mesh_data::Vertex::new(point[0], point[1], point[2]));
                        (vertices.len() - 1) as u32
                    })
                }));
            }
        }
        anyhow::ensure!(!faces.is_empty(), tr!("water-error-no-water"));
        let name = tr!("water-surface-name", surface = entry.surface.clone(), when = when);
        let built =
            crate::app::commands::triangulation::session::build_generated_triangulation(name.clone(), vertices, faces, crate::ui::state::TriSurfaceType::Surface, |_| Vec::new())?;
        self.insert_generated_triangulation(built);
        self.editor.toasts.push(tr!("water-surface-saved", name = name));
        Ok(())
    }
}

/// The water over `run`'s ground at `depth`, one surface per band that has
/// any.
fn water_bands(run: &WaterRun, depth: &[f32]) -> Vec<OpenTriangulation> {
    crate::model::water::water_surface(&run.ground, depth, crate::model::water::DRAWN_LIFT)
        .into_iter()
        .enumerate()
        .filter(|(_, triangles)| !triangles.is_empty())
        .filter_map(|(band, triangles)| {
            let faces: Vec<[u32; 3]> = (0..triangles.len() as u32 / 3).map(|face| [face * 3, face * 3 + 1, face * 3 + 2]).collect();
            let order: Vec<u32> = (0..faces.len() as u32).collect();
            let vertices = triangles.into_iter().map(|[x, y, z]| mesh_data::Vertex::new(x, y, z)).collect();
            let mesh = mesh_data::Triangulation::from_vertices_and_faces(vertices, faces).ok()?;
            Some(scene_surface(
                WATER_SURFACE_ID - 1 - band as u64,
                "",
                Arc::new(mesh),
                Arc::new(order),
                BAND_COLORS[band],
                BAND_COLORS[band],
            ))
        })
        .collect()
}
