//! Water runs: a storm over a surface, each in a tab of its own.
//!
//! Rain Simulation starts a run as a background job and opens its tab, in a
//! row under the viewport bar of the workspace it was started from. The
//! tab's viewport draws the run's own copy of the surface with the water on
//! it at the instant its timeline is at (`rendering/water.rs`), eased from
//! each kept instant to the next. The job hands over each instant as it is
//! worked out, so the tab plays what there is while the rest runs. A run
//! belongs to its tab: closing the tab stops it and discards it, and nothing
//! of it is saved with the project.

use std::sync::{Arc, Mutex, OnceLock, PoisonError};

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
        water::{Ground, WaterError, WaterFrame, WaterSettings},
    },
    rendering::water::WaterMesh,
    ui::state::{WaterRunStatus, WaterShown, WaterTab, Workspace},
};

/// The run's ground.
const WATER_SURFACE_ID: u64 = u64::MAX - 2_000_000;

enum RunState {
    Running,
    Failed(String),
    Ready,
}

/// The ground a run flows over, and the same as the mesh the viewport draws.
struct LiveGround {
    ground: Ground,
    mesh: Arc<mesh_data::Triangulation>,
    face_order: Arc<Vec<u32>>,
}

/// What a run's job has worked out so far, read by its tab while the job
/// carries on: the ground once it is welded and split, then each kept
/// instant as the storm reaches it.
#[derive(Default)]
struct LiveRun {
    ground: OnceLock<LiveGround>,
    frames: Mutex<Vec<Arc<WaterFrame>>>,
}

impl LiveRun {
    fn frames(&self) -> std::sync::MutexGuard<'_, Vec<Arc<WaterFrame>>> {
        self.frames.lock().unwrap_or_else(PoisonError::into_inner)
    }
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
    live: Arc<LiveRun>,
    /// The surface as drawn under the water: the source's own mesh until the
    /// run's ground is ready, then that.
    ground: OpenTriangulation,
    ground_landed: bool,
}

/// What the viewport is drawing for the open run: which run, and whether
/// its own ground had landed.
type SceneKey = (u64, bool);

/// What the water mesh was built from: which run, the kept instant at or
/// before the cursor, and whether the one after it had been worked out.
type MeshKey = (u64, usize, bool);

#[derive(Default)]
pub(crate) struct WaterRuns {
    runs: Vec<RunEntry>,
    next_id: u64,
    /// The open run's ground, as the viewport draws it.
    scene: Vec<OpenTriangulation>,
    scene_key: Option<SceneKey>,
    /// What the editor's water mesh was built from.
    mesh_key: Option<MeshKey>,
    meshes_built: u64,
}

impl WaterRuns {
    pub(crate) fn scene(&self) -> &[OpenTriangulation] {
        &self.scene
    }

    pub(crate) fn has(&self, id: u64) -> bool {
        self.runs.iter().any(|run| run.id == id)
    }
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

/// `surface` with the image `like` has draped over it.
fn draped_like(mut surface: OpenTriangulation, like: &OpenTriangulation) -> OpenTriangulation {
    surface.raster_texture = like.raster_texture;
    surface.raster_opacity = like.raster_opacity;
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
        let ground = draped_like(
            scene_surface(WATER_SURFACE_ID, &name, mesh.clone(), source.surface_face_order.clone(), source.color, source.line_color),
            source,
        );
        let title = tr!(
            "water-tab-title",
            surface = name.clone(),
            rain = plain_number(settings.rain_mm),
            hours = plain_number(settings.rain_hours)
        );

        let live = Arc::new(LiveRun::default());
        let shared = live.clone();
        let compute = move |cancel: &crate::app::jobs::CancelFlag, progress: &Progress| -> Result<()> {
            let signal = cancel.signal();
            let mut ground = Ground::from_mesh(&mesh);
            if let Some(longest) = settings.subdivide_m {
                ground = ground.subdivided(longest, &signal).map_err(water_error)?;
            }
            let mesh = Arc::new(ground.to_mesh().ok_or_else(|| water_error(WaterError::Empty))?);
            let face_order = Arc::new(crate::model::triangulation::spatial_surface_face_order(&mesh));
            let landed = shared.ground.get_or_init(|| LiveGround { ground, mesh, face_order });
            crate::model::water::simulate(&landed.ground, &settings, &crate::model::water::Boundaries::default(), &signal, progress, &mut |frame| {
                shared.frames().push(Arc::new(frame))
            })
            .map_err(water_error)
        };
        let progress = self.spawn_job_sharing_progress(
            tr!("water-job", surface = name.clone()),
            vec![JobKey::WaterRun(id)],
            compute,
            move |app, result: Result<()>| {
                let Some(entry) = app.water.runs.iter_mut().find(|entry| entry.id == id) else {
                    return;
                };
                entry.progress = None;
                match result {
                    Ok(()) => {
                        crate::userspace_log!(
                            "{}",
                            tr!(
                                "water-run-done",
                                title = entry.title.clone(),
                                vertices = entry.live.ground.get().map_or(0, |landed| landed.ground.vertices.len()).to_string(),
                                volume = format!("{:.0}", entry.live.frames().last().map_or(0.0, |frame| frame.on_surface))
                            )
                        );
                        entry.state = RunState::Ready;
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
            live,
            ground,
            ground_landed: false,
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
            .and_then(|id| self.water.runs.iter_mut().find(|run| run.id == id && run.workspace == workspace))
        else {
            self.editor.water.shown = None;
            self.editor.water.playing = false;
            self.editor.water.mesh = None;
            self.water.mesh_key = None;
            if self.water.scene_key.take().is_some() {
                self.water.scene = Vec::new();
                // The project's designs come back into the scene.
                self.scene_document_key = None;
                self.invalidate_geometry();
            }
            return;
        };

        // The ground the run flows over, once the job has made it.
        let live = entry.live.clone();
        if !entry.ground_landed
            && let Some(landed) = live.ground.get()
        {
            entry.ground = draped_like(
                scene_surface(
                    WATER_SURFACE_ID,
                    &entry.surface,
                    landed.mesh.clone(),
                    landed.face_order.clone(),
                    entry.ground.color,
                    entry.ground.line_color,
                ),
                &entry.ground,
            );
            entry.ground_landed = true;
        }
        let (id, settings) = (entry.id, entry.settings);
        let key: SceneKey = (id, entry.ground_landed);
        let running = matches!(entry.state, RunState::Running);
        let status = match &entry.state {
            RunState::Running => WaterRunStatus::Running(entry.progress.as_ref().and_then(|progress| progress.snapshot()).map(|snapshot| snapshot.fraction)),
            RunState::Failed(error) => WaterRunStatus::Failed(error.clone()),
            RunState::Ready => WaterRunStatus::Ready,
        };
        if self.water.scene_key != Some(key) {
            // The project's designs leave the scene when the tab opens.
            if self.water.scene_key.is_none() {
                self.scene_document_key = None;
            }
            self.water.scene = vec![entry.ground.clone()];
            self.water.scene_key = Some(key);
            self.invalidate_geometry();
        }

        // The kept instants either side of the cursor, of those worked out
        // so far: the cursor goes no further than the last of them.
        let (loaded_s, around) = {
            let frames = live.frames();
            let loaded_s = frames.last().map_or(0.0, |frame| frame.time_s);
            self.editor.water.time_s = self.editor.water.time_s.clamp(0.0, loaded_s);
            let time_s = self.editor.water.time_s;
            let around = frames.partition_point(|frame| frame.time_s <= time_s).checked_sub(1).map(|before| {
                let from = frames[before].clone();
                let next = frames.get(before + 1).cloned();
                (before, from, next)
            });
            (loaded_s, around)
        };
        let time_s = self.editor.water.time_s;
        self.editor.water.shown = Some(WaterShown {
            settings,
            status,
            instant_s: around.is_some().then_some(time_s),
            loaded_s,
        });
        // The tab is how the run's progress is seen.
        if running {
            self.redraw_requested = true;
        }

        let (Some(landed), Some((before, from, next))) = (live.ground.get(), around) else {
            self.editor.water.mesh = None;
            self.water.mesh_key = None;
            return;
        };
        let to = next.as_ref().unwrap_or(&from);
        self.editor.water.blend = if to.time_s > from.time_s {
            ((time_s - from.time_s) / (to.time_s - from.time_s)) as f32
        } else {
            0.0
        };
        self.editor.water.raining = time_s < settings.rain_hours * 3600.0;
        let mesh_key: MeshKey = (id, before, next.is_some());
        if self.water.mesh_key != Some(mesh_key) {
            self.water.meshes_built += 1;
            self.editor.water.mesh = Some(Arc::new(WaterMesh::build(
                self.water.meshes_built,
                &landed.ground,
                Some(&from.depth),
                &to.depth,
                next.as_ref().map_or(&[][..], |next| &next.arrival),
                settings.roughness,
            )));
            self.water.mesh_key = Some(mesh_key);
            self.redraw_requested = true;
        }
    }
}
