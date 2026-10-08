use std::{
    hash::{Hash, Hasher},
    io,
    sync::Arc,
};

use anyhow::Result;
use rayon::prelude::*;

use crate::{
    app::{
        App,
        commands::{file::FileDialogAction, triangulation::session::build_generated_triangulation},
        jobs::JobKey,
    },
    i18n::tr,
    model::{
        ItemRef, MemberKind, SceneEntityId, SectionKind,
        optimization::{AirMode, GridIssue, OptimizationScenario, ScenarioFile, unique_name},
        optimization_run::{
            self, RunInput, RunOutcome, RunResult,
            grid::{BlockLayout, Topography},
            prepare,
        },
        progress::Progress,
        project::ProjectItemState,
        triangulation::GeneratedTriangulation,
    },
    ui::state::{MAX_CONCURRENT_RUNS, RunState, ScenarioDraft, ShellStartPick, TriSurfaceType},
    userspace_error, userspace_log, userspace_warn,
};

impl<'a> App<'a> {
    /// Show the list. With a scenario open in the editor the list stays behind
    /// it: closing the editor is what asks about unsaved changes.
    ///
    /// The first time it opens in a session the saved scenarios file is read,
    /// so scenarios saved earlier are there without asking.
    pub(crate) fn open_optimization_scenarios(&mut self) {
        if !self.editor.optimization.loaded_from_file {
            self.editor.optimization.loaded_from_file = true;
            if let Err(error) = self.load_saved_optimization_scenarios() {
                userspace_warn!("{error:#}");
            }
        }
        self.editor.optimization.list_open = true;
    }

    /// Start a scenario in the editor. The list steps aside rather than staying
    /// open behind it: the editor is the only window of the pair at a time.
    pub(crate) fn add_optimization_scenario(&mut self) {
        let id = self.editor.optimization.fresh_id();
        let name = unique_name(
            self.editor.optimization.scenarios.iter().map(|scenario| scenario.name.as_str()),
            &tr!("opt-scenario-default-name", number = (self.editor.optimization.scenarios.len() + 1).to_string()),
        );
        self.editor.optimization.draft = Some(ScenarioDraft::new(OptimizationScenario::new(id, name), true));
        self.editor.optimization.list_open = false;
    }

    pub(crate) fn edit_optimization_scenario(&mut self, id: u64) {
        let Some(scenario) = self.editor.optimization.scenarios.iter().find(|scenario| scenario.id == id).cloned() else {
            return;
        };
        self.editor.optimization.draft = Some(ScenarioDraft::new(scenario, false));
        self.editor.optimization.list_open = false;
    }

    /// Put the edited scenario in the list (replacing its earlier copy) and
    /// write the file. The editor stays open, now holding it as saved, unless
    /// the user is closing. A failed write changes nothing the user can see, so
    /// the work is not lost.
    pub(crate) fn save_optimization_scenario(&mut self, mut scenario: OptimizationScenario, then_close: bool) -> Result<()> {
        let state = &mut self.editor.optimization;
        // The name is changed in the list only; the editor's copy never wins over it.
        if let Some(existing) = state.scenarios.iter().find(|existing| existing.id == scenario.id) {
            scenario.name = existing.name.clone();
        }
        match state.scenarios.iter_mut().find(|existing| existing.id == scenario.id) {
            Some(existing) => *existing = scenario.clone(),
            None => state.scenarios.push(scenario.clone()),
        }
        let location = self.write_optimization_scenarios()?;
        userspace_log!(
            "{}",
            tr!("opt-scenarios-saved-to", count = self.editor.optimization.scenarios.len().to_string(), location = location)
        );
        let state = &mut self.editor.optimization;
        if then_close {
            state.draft = None;
            state.list_open = true;
        } else if let Some(draft) = state.draft.as_mut().filter(|draft| draft.scenario.id == scenario.id) {
            draft.saved = scenario;
            draft.is_new = false;
            draft.confirm_close = false;
        }
        Ok(())
    }

    pub(crate) fn delete_optimization_scenario(&mut self, id: u64) -> Result<()> {
        self.editor.optimization.scenarios.retain(|scenario| scenario.id != id);
        self.editor.optimization.runs.remove(&id);
        self.write_optimization_scenarios().map(|_| ())
    }

    /// Copy a scenario to a new one right after it. The copy has not been run.
    pub(crate) fn duplicate_optimization_scenario(&mut self, id: u64) -> Result<()> {
        let state = &mut self.editor.optimization;
        let Some(position) = state.scenarios.iter().position(|scenario| scenario.id == id) else {
            return Ok(());
        };
        let mut copy = state.scenarios[position].clone();
        copy.id = state.fresh_id();
        copy.name = unique_name(
            state.scenarios.iter().map(|scenario| scenario.name.as_str()),
            &tr!("opt-copy-name", name = copy.name.clone()),
        );
        state.scenarios.insert(position + 1, copy);
        self.write_optimization_scenarios().map(|_| ())
    }

    pub(crate) fn rename_optimization_scenario(&mut self, id: u64, name: String) -> Result<()> {
        let name = name.trim().to_owned();
        match self.editor.optimization.scenarios.iter_mut().find(|scenario| scenario.id == id) {
            Some(scenario) if !name.is_empty() && scenario.name != name => scenario.name = name,
            _ => return Ok(()),
        }
        self.write_optimization_scenarios().map(|_| ())
    }

    /// Read the saved scenarios file into the list. A file that is not there
    /// yet is not an error: nothing has been saved.
    fn load_saved_optimization_scenarios(&mut self) -> Result<()> {
        match crate::app::io::load_optimization_scenarios() {
            Ok(file) => {
                let count = file.scenarios.len();
                self.editor.optimization.scenarios = file.scenarios;
                userspace_log!("{}", tr!("opt-scenarios-loaded", count = count.to_string()));
                Ok(())
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => anyhow::bail!("{}", tr!("opt-scenarios-load-failed", error = error.to_string())),
        }
    }

    /// Write the scenarios file, and say where it is.
    fn write_optimization_scenarios(&self) -> Result<String> {
        let file = ScenarioFile::new(self.editor.optimization.scenarios.clone());
        crate::app::io::save_optimization_scenarios(&file).map_err(|error| anyhow::anyhow!("{}", tr!("opt-scenarios-save-failed", error = error.to_string())))
    }

    /// Ask where to write the scenarios, and write them there.
    pub(crate) fn export_optimization_scenarios(&mut self) {
        let file = ScenarioFile::new(self.editor.optimization.scenarios.clone());
        #[cfg(target_arch = "wasm32")]
        match serde_json::to_vec_pretty(&file) {
            Ok(bytes) => Self::trigger_browser_download("optimization_scenarios.json".to_owned(), bytes, "application/json", "optimization scenarios"),
            Err(error) => userspace_warn!("{}", tr!("opt-scenarios-save-failed", error = error.to_string())),
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = file;
            self.spawn_file_dialog(async {
                let handle = rfd::AsyncFileDialog::new()
                    .add_filter("JSON", &["json"])
                    .set_file_name("optimization_scenarios.json")
                    .save_file()
                    .await?;
                Some(FileDialogAction::ExportOptimizationScenarios(handle.path().to_owned()))
            });
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn write_optimization_scenarios_export(&mut self, mut path: std::path::PathBuf) -> Result<()> {
        if path.extension().is_none() {
            path.set_extension("json");
        }
        let file = ScenarioFile::new(self.editor.optimization.scenarios.clone());
        crate::app::io::export_optimization_scenarios(&path, &file).map_err(|error| anyhow::anyhow!("{}", tr!("opt-scenarios-save-failed", error = error.to_string())))?;
        userspace_log!(
            "{}",
            tr!("opt-scenarios-exported", count = file.scenarios.len().to_string(), path = path.display().to_string())
        );
        Ok(())
    }

    /// Ask for a scenarios file and add what is in it to the list.
    pub(crate) fn import_optimization_scenarios(&mut self) {
        #[cfg(target_arch = "wasm32")]
        self.spawn_file_dialog(async {
            let handle = rfd::AsyncFileDialog::new().add_filter("JSON", &["json"]).pick_file().await?;
            Some(FileDialogAction::WebImportOptimizationScenarios(crate::model::input::read_browser_handle(handle).await))
        });
        #[cfg(not(target_arch = "wasm32"))]
        self.spawn_file_dialog(async {
            let handle = rfd::AsyncFileDialog::new().add_filter("JSON", &["json"]).pick_file().await?;
            Some(FileDialogAction::ImportOptimizationScenarios(handle.path().to_owned()))
        });
    }

    /// Add the scenarios in `text` to the list, each with a fresh id and a name
    /// the list does not already use. They have not been run.
    pub(crate) fn merge_imported_optimization_scenarios(&mut self, text: &str) -> Result<()> {
        let file = crate::app::io::parse_optimization_scenarios(text).map_err(|error| anyhow::anyhow!("{}", tr!("opt-scenarios-import-failed", error = error.to_string())))?;
        let count = file.scenarios.len();
        let state = &mut self.editor.optimization;
        for mut scenario in file.scenarios {
            scenario.id = state.fresh_id();
            scenario.name = unique_name(state.scenarios.iter().map(|existing| existing.name.as_str()), &scenario.name);
            state.scenarios.push(scenario);
        }
        state.list_open = true;
        self.write_optimization_scenarios()?;
        userspace_log!("{}", tr!("opt-scenarios-imported", count = count.to_string()));
        Ok(())
    }

    /// Ask for the folder the open scenario's reports go to.
    pub(crate) fn choose_optimization_reports_folder(&mut self) {
        #[cfg(target_arch = "wasm32")]
        userspace_warn!("{}", tr!("opt-reports-browser-unavailable"));
        #[cfg(not(target_arch = "wasm32"))]
        self.spawn_file_dialog(async {
            let handle = rfd::AsyncFileDialog::new().pick_folder().await?;
            Some(FileDialogAction::OptimizationReportsFolder(handle.path().to_owned()))
        });
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn set_optimization_reports_folder(&mut self, path: std::path::PathBuf) {
        if let Some(draft) = self.editor.optimization.draft.as_mut() {
            draft.scenario.output.reports_folder = path.display().to_string();
        }
    }

    /// Start picking the directional shells' starting point: everything but the
    /// scenario's block model is hidden, the view goes to plan fitted to the
    /// model, and the editor steps aside until a block is clicked.
    pub(crate) fn begin_shell_start_pick(&mut self) {
        let Some(name) = self.editor.optimization.draft.as_ref().map(|draft| draft.scenario.block_model.clone()) else {
            return;
        };
        let Some((block_model, marker_z, loaded)) = self
            .block_models
            .iter()
            .find(|model| model.name == name)
            .map(|model| (model.id, model.world_bounds().map_or(0.0, |(_, max)| max.z), model.state.loaded))
        else {
            userspace_warn!("{}", tr!("opt-pick-needs-block-model"));
            return;
        };
        // A hidden model is shown for the pick (read back in first if it was
        // moved out of memory) and hidden again afterwards.
        if !loaded {
            self.set_item_loaded(ItemRef::BlockModel(block_model), true);
        }
        let keep = SceneEntityId::BlockModel(block_model);
        let previously_hidden = self.editor.hidden_handles.clone();
        let everything = self
            .scene_document
            .objects()
            .iter()
            .map(|object| SceneEntityId::Object(object.id()))
            .chain(self.triangulations.iter().map(|item| SceneEntityId::Triangulation(item.id)))
            .chain(self.point_clouds.iter().map(|item| SceneEntityId::PointCloud(item.id)))
            .chain(self.drill_holes.iter().map(|item| SceneEntityId::DrillHole(item.id)))
            .chain(self.block_models.iter().map(|item| SceneEntityId::BlockModel(item.id)))
            .filter(|entity| *entity != keep);
        self.editor.hidden_handles.extend(everything);
        self.editor.hidden_handles.remove(&keep);
        self.editor.optimization.start_pick = Some(ShellStartPick {
            block_model,
            previously_hidden,
            marker_z,
            hide_after: !loaded,
            loading: !loaded,
        });
        self.invalidate_geometry();
        self.invalidate_overlay();
        self.fit_view_for_shell_start_pick();
    }

    /// Plan view and zoom to all, on what is left showing.
    fn fit_view_for_shell_start_pick(&mut self) {
        if let Some(graphics) = self.graphics.as_mut() {
            graphics.fit_to_extents(
                &self.scene_document,
                &self.triangulations,
                &self.block_models,
                &self.drill_holes,
                &self.point_clouds,
                &self.editor.hidden_handles,
            );
        }
        self.redraw_requested = true;
    }

    /// Put visibility back and bring the editor back, without a point.
    pub(crate) fn cancel_shell_start_pick(&mut self) {
        self.end_shell_start_pick();
    }

    fn end_shell_start_pick(&mut self) -> bool {
        let Some(pick) = self.editor.optimization.start_pick.take() else {
            return false;
        };
        self.editor.hidden_handles = pick.previously_hidden;
        self.editor.viewport_pick_hover_label = None;
        if pick.hide_after && self.block_models.iter().any(|model| model.id == pick.block_model) {
            self.set_item_loaded(ItemRef::BlockModel(pick.block_model), false);
        }
        self.invalidate_geometry();
        self.invalidate_overlay();
        true
    }

    /// A click while picking: a block of the scenario's model sets the point.
    pub(crate) fn pick_shell_start_at_cursor(&mut self) {
        let Some(block_model) = self.editor.optimization.start_pick.as_ref().map(|pick| pick.block_model) else {
            return;
        };
        let picked = self.graphics.as_ref().and_then(|graphics| {
            graphics.pick_scene_entity_at_cursor(
                crate::app::PICK_THRESHOLD_PX,
                &self.triangulations,
                &self.drill_holes,
                &self.editor.hidden_handles,
                &self.editor.frozen_handles,
                false,
            )
        });
        if let Some(pick) = picked
            && pick.entity == SceneEntityId::BlockModel(block_model)
        {
            let world = pick.world;
            if let Some(draft) = self.editor.optimization.draft.as_mut() {
                draft.scenario.output.shell_start = Some((world.x, world.y));
            }
            self.end_shell_start_pick();
            userspace_log!("{}", tr!("opt-pick-done", x = format!("{:.2}", world.x), y = format!("{:.2}", world.y)));
        }
    }
}

// ── Runs ──

/// A run's output, built on the worker: shell meshes are already
/// triangulations with their spatial index.
struct FinishedRun {
    outcome: RunOutcome,
    triangulations: Vec<ShellItem>,
}

/// One shell's triangulation, ready to insert.
struct ShellItem {
    /// 1-based.
    shell: usize,
    solid: bool,
    built: GeneratedTriangulation,
}

impl<'a> App<'a> {
    /// Run the saved scenario `id`, or queue it while enough others run.
    pub(crate) fn run_optimization_scenario(&mut self, id: u64) {
        let state = &mut self.editor.optimization;
        let Some(scenario) = state.scenarios.iter().find(|scenario| scenario.id == id).cloned() else {
            return;
        };
        if matches!(state.runs.get(&id), Some(RunState::Running { .. } | RunState::Queued)) {
            return;
        }
        if state.running_count() >= MAX_CONCURRENT_RUNS {
            state.runs.insert(id, RunState::Queued);
            userspace_log!("{}", tr!("opt-run-queued", name = scenario.name.clone()));
            return;
        }
        self.start_optimization_run(scenario);
    }

    /// Stop a run (its result is thrown away) or take it out of the queue.
    pub(crate) fn cancel_optimization_scenario(&mut self, id: u64) {
        self.cancel_jobs(|key| *key == JobKey::OptimizationRun(id));
        let state = &mut self.editor.optimization;
        state.awaiting_restore.remove(&id);
        if state.runs.remove(&id).is_some()
            && let Some(scenario) = state.scenarios.iter().find(|scenario| scenario.id == id)
        {
            userspace_log!("{}", tr!("opt-run-cancelled", name = scenario.name.clone()));
        }
        self.start_queued_optimization_runs();
    }

    /// Start queued scenarios while there is room.
    fn start_queued_optimization_runs(&mut self) {
        loop {
            let state = &self.editor.optimization;
            if state.running_count() >= MAX_CONCURRENT_RUNS {
                return;
            }
            // Queued in list order.
            let next = state
                .scenarios
                .iter()
                .find(|scenario| matches!(state.runs.get(&scenario.id), Some(RunState::Queued)))
                .cloned();
            let Some(scenario) = next else {
                return;
            };
            self.editor.optimization.runs.remove(&scenario.id);
            self.start_optimization_run(scenario);
        }
    }

    /// Follow a hidden block model being shown for a pick: fit the view once it
    /// is there, or warn and end the pick if it could not be shown.
    pub(crate) fn watch_shell_start_pick_load(&mut self) {
        let Some(pick) = self.editor.optimization.start_pick.as_ref().filter(|pick| pick.loading) else {
            return;
        };
        let item = ItemRef::BlockModel(pick.block_model);
        let loaded = self.block_models.iter().find(|model| model.id == pick.block_model).map(|model| model.state.loaded);
        match loaded {
            Some(true) => {
                if let Some(pick) = self.editor.optimization.start_pick.as_mut() {
                    pick.loading = false;
                }
                self.invalidate_geometry();
                self.fit_view_for_shell_start_pick();
            }
            Some(false) if self.item_load_pending(item) || self.restore_pending() => {}
            _ => {
                userspace_warn!("{}", tr!("opt-pick-show-failed"));
                self.end_shell_start_pick();
            }
        }
    }

    /// Drop the state of runs whose job is gone without applying (its block
    /// model closed, say), so they do not show as running for ever.
    pub(crate) fn reconcile_optimization_runs(&mut self) {
        let restoring = self.restore_pending();
        let lost: Vec<u64> = self
            .editor
            .optimization
            .runs
            .iter()
            .filter(|(id, run)| {
                matches!(run, RunState::Running { .. })
                    && !self.job_pending(&JobKey::OptimizationRun(**id))
                    && !(restoring && self.editor.optimization.awaiting_restore.contains(id))
            })
            .map(|(id, _)| *id)
            .collect();
        if lost.is_empty() {
            return;
        }
        for id in lost {
            self.editor.optimization.runs.remove(&id);
            self.editor.optimization.awaiting_restore.remove(&id);
        }
        self.start_queued_optimization_runs();
    }

    fn fail_optimization_run(&mut self, id: u64, name: &str, issues: &[String]) {
        userspace_error!("{}", tr!("opt-run-cannot-start", name = name.to_owned()));
        for issue in issues {
            userspace_error!("{}", tr!("opt-run-issue", issue = issue.clone()));
        }
        self.editor.optimization.runs.remove(&id);
        self.editor.optimization.awaiting_restore.remove(&id);
        self.start_queued_optimization_runs();
    }

    /// Check the scenario, read its block model and topography back in if they
    /// were moved out of memory, and start the job.
    fn start_optimization_run(&mut self, scenario: OptimizationScenario) {
        let id = scenario.id;
        // Hidden (unloaded) models run too: their values are read back below.
        let Some(model_index) = self.block_models.iter().position(|model| model.name == scenario.block_model) else {
            let issue = tr!("opt-run-needs-block-model", name = scenario.name.clone(), model = scenario.block_model.clone());
            self.fail_optimization_run(id, &scenario.name, &[issue]);
            return;
        };
        let topography = (scenario.exclude_air && scenario.air_mode == AirMode::Topography)
            .then(|| self.triangulations.iter().position(|item| item.name == scenario.air_topography))
            .flatten();

        let mut needed = vec![ItemRef::BlockModel(self.block_models[model_index].id)];
        needed.extend(topography.map(|index| ItemRef::Triangulation(self.triangulations[index].id)));
        let deferred = {
            let state = &mut self.editor.optimization;
            state.runs.insert(id, RunState::Running { progress: Progress::new() });
            state.awaiting_restore.insert(id);
            let scenario = scenario.clone();
            self.restore_items_for(needed, move |app| {
                if matches!(app.editor.optimization.runs.get(&id), Some(RunState::Running { .. })) {
                    app.launch_optimization_run(scenario);
                }
            })
        };
        if !deferred {
            self.launch_optimization_run(scenario);
        }
    }

    /// Everything the scenario needs is in memory: prepare it and spawn the job.
    fn launch_optimization_run(&mut self, scenario: OptimizationScenario) {
        let id = scenario.id;
        self.editor.optimization.awaiting_restore.remove(&id);
        let Some(model) = self.block_models.iter().find(|model| model.name == scenario.block_model) else {
            let issue = tr!("opt-run-needs-block-model", name = scenario.name.clone(), model = scenario.block_model.clone());
            self.fail_optimization_run(id, &scenario.name, &[issue]);
            return;
        };
        if let Some(issue) = GridIssue::of(model) {
            self.fail_optimization_run(id, &scenario.name, &[issue.message()]);
            return;
        }
        let topography = (scenario.exclude_air && scenario.air_mode == AirMode::Topography)
            .then(|| self.triangulations.iter().find(|item| item.name == scenario.air_topography))
            .flatten()
            .map(|item| Topography {
                mesh: Arc::clone(&item.mesh),
                spatial: Arc::clone(&item.spatial),
            });
        let prepared = match prepare::prepare(&scenario, &model.model, topography) {
            Ok(prepared) => prepared,
            Err(issues) => {
                self.fail_optimization_run(id, &scenario.name, &issues);
                return;
            }
        };
        let layout = model
            .uniform_grid
            .clone()
            .and_then(|uniform| BlockLayout::new(Arc::clone(&model.blocks), uniform, |local| model.model.local_to_world(local)));
        let Some(layout) = layout else {
            self.fail_optimization_run(id, &scenario.name, &[GridIssue::Irregular.message()]);
            return;
        };
        let model_id = model.id;
        let cells = layout.grid.cell_count();
        let output = &scenario.output;
        let input = RunInput {
            scenario_name: scenario.name.clone(),
            layer_name: output.shell_layer_name.trim().to_owned(),
            prepared,
            layout,
            make_solids: output.shell_as_solid,
            make_surfaces: output.shell_as_surface,
        };
        if output.create_reports || output.write_shell_field {
            userspace_log!("{}", tr!("opt-run-outputs-stub"));
        }
        userspace_log!("{}", tr!("opt-run-started", name = scenario.name.clone(), cells = cells.to_string()));

        let fingerprint = scenario.fingerprint();
        let started = web_time::Instant::now();
        let progress = self.spawn_job_sharing_progress(
            tr!("opt-run-label", name = scenario.name.clone()),
            vec![JobKey::OptimizationRun(id), JobKey::BlockModel(model_id)],
            move |cancel, progress| {
                let outcome = optimization_run::run(input, cancel, progress)?;
                let triangulations = outcome
                    .meshes
                    .par_iter()
                    .map(|shell| {
                        let mesh = &shell.mesh;
                        let edges = mesh.edges.clone();
                        let kind = if shell.solid { TriSurfaceType::SolidClosed } else { TriSurfaceType::Surface };
                        build_generated_triangulation(shell.name.clone(), mesh.vertices.clone(), mesh.faces.clone(), kind, |_| edges).map(|built| ShellItem {
                            shell: shell.shell,
                            solid: shell.solid,
                            built,
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;
                Ok(FinishedRun { outcome, triangulations })
            },
            move |app, result| app.finish_optimization_run(scenario, fingerprint, started, result),
        );
        self.editor.optimization.runs.insert(id, RunState::Running { progress });
    }

    fn finish_optimization_run(&mut self, scenario: OptimizationScenario, fingerprint: u64, started: web_time::Instant, result: Result<FinishedRun>) {
        let id = scenario.id;
        match result {
            Ok(FinishedRun { outcome, triangulations }) => {
                self.editor.optimization.runs.insert(id, RunState::Finished { fingerprint });
                let summaries = &outcome.shells.summaries;
                userspace_log!(
                    "{}",
                    tr!(
                        "opt-run-finished",
                        name = scenario.name.clone(),
                        count = summaries.len().to_string(),
                        seconds = format!("{:.1}", started.elapsed().as_secs_f64())
                    )
                );
                if outcome.blocks_with_default_angle > 0 {
                    userspace_warn!("{}", tr!("opt-run-default-angle", count = outcome.blocks_with_default_angle.to_string()));
                }
                if outcome.blocks_with_default_density > 0 {
                    userspace_warn!("{}", tr!("opt-run-no-density", count = outcome.blocks_with_default_density.to_string()));
                }
                for (index, summary) in summaries.iter().enumerate() {
                    userspace_log!(
                        "{}",
                        tr!(
                            "opt-run-shell-summary",
                            shell = (index + 1).to_string(),
                            factor = match summary.distance {
                                None => crate::model::optimization::format_factor(summary.factor),
                                Some(share) => format!("{}, {:.0}%", crate::model::optimization::format_factor(summary.factor), share * 100.0),
                            },
                            blocks = summary.blocks.to_string(),
                            tonnes = format!("{:.0}", summary.tonnes),
                            ore = format!("{:.0}", summary.ore_tonnes),
                            waste = format!("{:.0}", summary.waste_tonnes),
                            value = format!("{:.0}", summary.value)
                        )
                    );
                }
                self.add_shell_triangulations(&scenario, started, triangulations);
                self.editor.optimization.results.insert(
                    id,
                    Arc::new(RunResult {
                        grid: outcome.grid,
                        shells: outcome.shells,
                    }),
                );
            }
            Err(error) => {
                self.editor.optimization.runs.remove(&id);
                if format!("{error:#}").contains("Cancelled") {
                    userspace_log!("{}", tr!("opt-run-cancelled", name = scenario.name.clone()));
                } else {
                    userspace_error!("{}", tr!("opt-run-failed", name = scenario.name.clone(), error = format!("{error:#}")));
                }
            }
        }
        self.start_queued_optimization_runs();
    }

    /// The shells as triangulations, each a random colour (a shell's solid and
    /// surface share one): solids in a collection named after the scenario and
    /// the layer name, surfaces in one with " surface" after that.
    fn add_shell_triangulations(&mut self, scenario: &OptimizationScenario, started: web_time::Instant, triangulations: Vec<ShellItem>) {
        if triangulations.is_empty() {
            return;
        }
        let solids = format!("{} {}", scenario.name, scenario.output.shell_layer_name.trim()).trim().to_owned();
        let surfaces = format!("{solids} {}", tr!("opt-shell-surface-suffix"));
        let mut folder = |name: &str| {
            self.workspace
                .active_project_mut()
                .and_then(|project| project.project.folders.ensure(SectionKind::Triangulations, name))
        };
        // A collection only for a kind the run made, so none is left empty.
        let solid_folder = triangulations.iter().any(|item| item.solid).then(|| folder(&solids)).flatten();
        let surface_folder = triangulations.iter().any(|item| !item.solid).then(|| folder(&surfaces)).flatten();
        let seed = {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            (scenario.id, started.elapsed().as_nanos(), self.triangulations.len()).hash(&mut hasher);
            hasher.finish()
        };
        for ShellItem { shell, solid, built } in triangulations {
            let state = ProjectItemState::dirty(MemberKind::Triangulation, None)
                .with_section(SectionKind::natural_for(MemberKind::Triangulation))
                .with_folder(if solid { solid_folder } else { surface_folder });
            self.insert_generated_triangulation_with(built, state, shell_color(seed, shell), false);
        }
        self.invalidate_geometry();
    }
}

/// A random, easily told apart colour for a shell: hues a golden angle apart
/// from a random start, at a calm saturation.
fn shell_color(seed: u64, shell: usize) -> [f32; 4] {
    let hue = ((seed % 360) as f32 / 360.0 + shell as f32 * 0.618_034).fract();
    let (saturation, value) = (0.55, 0.85);
    let sector = hue * 6.0;
    let chroma = value * saturation;
    let x = chroma * (1.0 - (sector % 2.0 - 1.0).abs());
    let (r, g, b) = match sector as u32 {
        0 => (chroma, x, 0.0),
        1 => (x, chroma, 0.0),
        2 => (0.0, chroma, x),
        3 => (0.0, x, chroma),
        4 => (x, 0.0, chroma),
        _ => (chroma, 0.0, x),
    };
    let base = value - chroma;
    [r + base, g + base, b + base, 1.0]
}
