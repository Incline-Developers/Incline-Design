use std::{
    collections::{BTreeMap, HashMap},
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
        block_model::{BlockModelId, ColorTransferFunction},
        folders::FolderId,
        optimization::{AirMode, BlockModelFields, GridIssue, OptimizationScenario, ScenarioFile, unique_name},
        optimization_run::{
            self, RunInput, RunOutcome, RunResult, ShellField,
            grid::{BlockLayout, Topography},
            prepare,
            report::{self, Basis},
        },
        progress::Progress,
        project::ProjectItemState,
        triangulation::{GeneratedTriangulation, TriangulationId},
    },
    ui::state::{MAX_CONCURRENT_RUNS, ResultsSource, ResultsView, RunState, ScenarioDraft, ShellStartPick, TriSurfaceType},
    userspace_error, userspace_log, userspace_warn,
};

impl<'a> App<'a> {
    /// Show the list. With a scenario open in the editor the list stays behind
    /// it: closing the editor is what asks about unsaved changes.
    ///
    /// The first time it opens in a session the saved scenarios file is read,
    /// so scenarios saved earlier are there without asking.
    pub(crate) fn open_optimization_scenarios(&mut self) {
        self.ensure_optimization_scenarios_loaded();
        self.editor.optimization.list_open = true;
    }

    /// Read the saved scenarios file once a session, before anything that
    /// shows or writes it - a write before the read would replace the saved
    /// scenarios with an empty list.
    fn ensure_optimization_scenarios_loaded(&mut self) {
        if !self.editor.optimization.loaded_from_file {
            self.editor.optimization.loaded_from_file = true;
            if let Err(error) = self.load_saved_optimization_scenarios() {
                userspace_warn!("{error:#}");
            }
        }
    }

    /// Keep the Results table's column choices, with the scenarios.
    pub(crate) fn save_optimization_report_columns(&mut self) -> Result<()> {
        if !self.editor.optimization.loaded_from_file {
            // The choices made before the read would be lost to it; keep them.
            let chosen = std::mem::take(&mut self.editor.optimization.report_columns);
            self.ensure_optimization_scenarios_loaded();
            self.editor.optimization.report_columns.extend(chosen);
        }
        self.write_optimization_scenarios().map(|_| ())
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
            Some(scenario) if !name.is_empty() && scenario.name != name => {
                // The report's column choices follow the scenario's name.
                let old = std::mem::replace(&mut scenario.name, name.clone());
                let columns = &mut self.editor.optimization.report_columns;
                if let Some(chosen) = columns.remove(&old) {
                    columns.insert(name, chosen);
                }
            }
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
                self.editor.optimization.report_columns = file.report_columns;
                userspace_log!("{}", tr!("opt-scenarios-loaded", count = count.to_string()));
                Ok(())
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => anyhow::bail!("{}", tr!("opt-scenarios-load-failed", error = error.to_string())),
        }
    }

    /// Write the scenarios file, and say where it is.
    fn write_optimization_scenarios(&self) -> Result<String> {
        let mut file = ScenarioFile::new(self.editor.optimization.scenarios.clone());
        file.report_columns = self.editor.optimization.report_columns.clone();
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
    /// The report as CSV, when reports are made.
    csv: Option<Vec<u8>>,
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
        // Checked before the run, so a field that cannot be written does not
        // cost the run's time first.
        let shell_field = match scenario.shell_field_target(&BlockModelFields::of_open(model)) {
            Ok(field) => field,
            Err(issue) => {
                self.fail_optimization_run(id, &scenario.name, &[issue]);
                return;
            }
        };
        // A browser downloads the report instead, so needs no folder.
        if cfg!(not(target_arch = "wasm32")) && scenario.output.create_reports && scenario.output.reports_folder.trim().is_empty() {
            self.fail_optimization_run(id, &scenario.name, &[tr!("opt-run-reports-folder-missing")]);
            return;
        }
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
            field_value: shell_field.is_some().then_some(output.shell_field_value),
        };
        let create_reports = output.create_reports;
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
                let csv = create_reports.then(|| report::write_csv(&outcome.report)).transpose()?;
                Ok(FinishedRun { outcome, triangulations, csv })
            },
            move |app, result| app.finish_optimization_run(scenario, fingerprint, started, model_id, shell_field, result),
        );
        self.editor.optimization.runs.insert(id, RunState::Running { progress });
    }

    fn finish_optimization_run(
        &mut self,
        scenario: OptimizationScenario,
        fingerprint: u64,
        started: web_time::Instant,
        model_id: BlockModelId,
        shell_field: Option<String>,
        result: Result<FinishedRun>,
    ) {
        let id = scenario.id;
        match result {
            Ok(FinishedRun { mut outcome, triangulations, csv }) => {
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
                // Each shell's pit at the base price, as the report has it.
                let report = &outcome.report;
                for (index, shell) in report.shells.iter().enumerate() {
                    let pit = report.bucket(Basis::Cumulative, index, None);
                    let (ore, waste) = report.ore_waste(Basis::Cumulative, index);
                    userspace_log!(
                        "{}",
                        tr!(
                            "opt-run-shell-summary",
                            shell = shell.number.to_string(),
                            factor = match shell.distance {
                                None => crate::model::optimization::format_factor(shell.factor),
                                Some(share) => format!("{}, {:.0}%", crate::model::optimization::format_factor(shell.factor), share * 100.0),
                            },
                            blocks = pit.blocks.to_string(),
                            tonnes = format!("{:.0}", pit.tonnes),
                            ore = format!("{ore:.0}"),
                            waste = format!("{waste:.0}"),
                            value = format!("{:.0}", pit.cash_flow())
                        )
                    );
                }
                if let Some(bytes) = csv {
                    self.write_optimization_report(&scenario, bytes);
                }
                // One seed for the shells' colours and the field's, so a shell
                // and its blocks share a colour.
                let seed = {
                    let mut hasher = std::collections::hash_map::DefaultHasher::new();
                    (scenario.id, started.elapsed().as_nanos(), self.triangulations.len()).hash(&mut hasher);
                    hasher.finish()
                };
                let colors = self.add_shell_triangulations(&scenario, seed, triangulations);
                if let (Some(field), Some(values)) = (shell_field, outcome.field.take()) {
                    self.write_shell_field(model_id, field, values, colors, seed);
                }
                self.editor.optimization.results.insert(
                    id,
                    Arc::new(RunResult {
                        report: Arc::new(outcome.report),
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
    /// the layer name, surfaces in one with " surface" after that. Returns each
    /// shell's colour (1-based), kept from an updated item or newly given.
    fn add_shell_triangulations(&mut self, scenario: &OptimizationScenario, seed: u64, triangulations: Vec<ShellItem>) -> HashMap<usize, [f32; 4]> {
        let mut colors = HashMap::new();
        if triangulations.is_empty() {
            return colors;
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
        // A rerun updates the shells of the same name in place, keeping their
        // colours, and removes those it no longer makes, so each collection
        // always holds the latest run.
        let in_folders = |app: &Self| -> Vec<(TriangulationId, Option<FolderId>, String)> {
            app.triangulations
                .iter()
                .filter(|item| item.state.folder.is_some() && (item.state.folder == solid_folder || item.state.folder == surface_folder))
                .map(|item| (item.id, item.state.folder, item.name.clone()))
                .collect()
        };
        let mut stale = in_folders(self);
        for ShellItem { shell, solid, built } in triangulations {
            let folder = if solid { solid_folder } else { surface_folder };
            if let Some(index) = stale.iter().position(|(_, existing, name)| *existing == folder && *name == built.name) {
                let (id, ..) = stale.swap_remove(index);
                self.replace_generated_triangulation(id, built);
                if let Some(item) = self.triangulations.iter().find(|item| item.id == id) {
                    colors.entry(shell).or_insert(item.color);
                }
                continue;
            }
            let state = ProjectItemState::dirty(MemberKind::Triangulation, None)
                .with_section(SectionKind::natural_for(MemberKind::Triangulation))
                .with_folder(folder);
            let color = *colors.entry(shell).or_insert_with(|| shell_color(seed, shell));
            self.insert_generated_triangulation_with(built, state, color, false);
        }
        for (id, ..) in stale {
            self.remove_triangulation(id);
        }
        self.evict_unloaded_items();
        self.invalidate_geometry();
        colors
    }

    /// Write a run's shells into categorical field `field` of block model
    /// `model_id`, adding the field or overwriting it. A block model moved out
    /// of memory since the run is read back first, and leaves again after.
    fn write_shell_field(&mut self, model_id: BlockModelId, field: String, values: ShellField, colors: HashMap<usize, [f32; 4]>, seed: u64) {
        let restoring = self.restore_items_for(vec![ItemRef::BlockModel(model_id)], {
            let (field, values, colors) = (field.clone(), values.clone(), colors.clone());
            move |app| app.apply_shell_field(model_id, &field, values, &colors, seed)
        });
        if !restoring {
            self.apply_shell_field(model_id, &field, values, &colors, seed);
        }
    }

    /// Each category's colour: the one its name already had in the field (so a
    /// rerun, or a colour the user changed, stays), else its shell's
    /// triangulation colour, else a new random one.
    fn apply_shell_field(&mut self, model_id: BlockModelId, field: &str, values: ShellField, shell_colors: &HashMap<usize, [f32; 4]>, seed: u64) {
        let Some(model) = self.block_models.iter_mut().find(|model| model.id == model_id) else {
            return;
        };
        let earlier: HashMap<String, [f32; 4]> = model
            .model
            .variable(field)
            .map(|variable| {
                let gradient = match model.color_transfers.get(field) {
                    Some(ColorTransferFunction::Category { gradient }) => gradient.clone(),
                    _ => variable.category_colors.clone(),
                };
                variable
                    .strings
                    .iter()
                    .filter_map(|(code, name)| gradient.get(code).map(|color| (name.clone(), *color)))
                    .collect()
            })
            .unwrap_or_default();
        let colors: BTreeMap<u32, [f32; 4]> = values
            .categories
            .iter()
            .map(|(&code, name)| {
                let shell = code as usize;
                let color = earlier.get(name).or_else(|| shell_colors.get(&shell)).copied().unwrap_or_else(|| shell_color(seed, shell));
                (code, color)
            })
            .collect();
        let count = values.categories.len();
        if let Err(error) = model.model.set_category_column(field, values.codes, values.categories, colors.clone()) {
            userspace_error!(
                "{}",
                tr!("opt-run-field-failed", field = field.to_owned(), model = model.name.clone(), error = error.to_string())
            );
            return;
        }
        model.color_transfers.insert(field.to_owned(), ColorTransferFunction::Category { gradient: colors });
        if model.active_color_variable.as_deref() == Some(field) {
            model.clear_active_values_cache();
        }
        model.state.touch();
        let name = model.name.clone();
        self.touch_active_project_content();
        userspace_log!("{}", tr!("opt-run-field-written", field = field.to_owned(), model = name, count = count.to_string()));
        self.evict_unloaded_items();
        self.invalidate_geometry();
    }
}

impl<'a> App<'a> {
    /// Write a run's report: on the desktop into `<reports folder>/<scenario>/`,
    /// replacing only the report file (the folder and anything else in it are
    /// left alone); in the browser, as a download.
    fn write_optimization_report(&mut self, scenario: &OptimizationScenario, bytes: Vec<u8>) {
        #[cfg(target_arch = "wasm32")]
        Self::trigger_browser_download(format!("{}_{}", folder_name(&scenario.name), report::FILE_NAME), bytes, "text/csv", "optimization report");
        #[cfg(not(target_arch = "wasm32"))]
        {
            let folder = std::path::Path::new(scenario.output.reports_folder.trim()).join(folder_name(&scenario.name));
            let path = folder.join(report::FILE_NAME);
            let written = std::fs::create_dir_all(&folder)
                .map_err(anyhow::Error::from)
                .and_then(|()| crate::model::atomic_file::write_atomic(&path, |file| Ok(io::Write::write_all(file, &bytes)?)));
            match written {
                Ok(()) => userspace_log!("{}", tr!("opt-run-report-written", path = path.display().to_string())),
                Err(error) => userspace_error!("{}", tr!("opt-run-report-failed", path = path.display().to_string(), error = format!("{error:#}"))),
            }
        }
    }
}

impl<'a> App<'a> {
    /// Save the report the Results window shows as CSV: a file the user
    /// chooses, or a download in the browser.
    pub(crate) fn export_optimization_report(&mut self) {
        let Some((name, report)) = self.editor.optimization.results_report() else {
            return;
        };
        let bytes = match report::write_csv(&report) {
            Ok(bytes) => bytes,
            Err(error) => {
                userspace_error!("{}", tr!("opt-results-export-failed", error = format!("{error:#}")));
                return;
            }
        };
        let file_name = format!("{}_{}", folder_name(&name), report::FILE_NAME);
        #[cfg(target_arch = "wasm32")]
        Self::trigger_browser_download(file_name, bytes, "text/csv", "optimization report");
        #[cfg(not(target_arch = "wasm32"))]
        self.spawn_file_dialog(async move {
            let handle = rfd::AsyncFileDialog::new().add_filter("CSV", &["csv"]).set_file_name(file_name).save_file().await?;
            Some(FileDialogAction::ExportOptimizationReport(handle.path().to_owned(), bytes))
        });
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn write_optimization_report_export(&mut self, mut path: std::path::PathBuf, bytes: Vec<u8>) -> Result<()> {
        if path.extension().is_none() {
            path.set_extension("csv");
        }
        crate::model::atomic_file::write_atomic(&path, |file| Ok(io::Write::write_all(file, &bytes)?))
            .map_err(|error| anyhow::anyhow!("{}", tr!("opt-results-export-failed", error = format!("{error:#}"))))?;
        userspace_log!("{}", tr!("opt-results-exported", path = path.display().to_string()));
        Ok(())
    }

    /// Ask for a report CSV and show it in the Results window.
    pub(crate) fn open_optimization_report(&mut self) {
        #[cfg(target_arch = "wasm32")]
        self.spawn_file_dialog(async {
            let handle = rfd::AsyncFileDialog::new().add_filter("CSV", &["csv"]).pick_file().await?;
            Some(FileDialogAction::WebOpenOptimizationReport(crate::model::input::read_browser_handle(handle).await))
        });
        #[cfg(not(target_arch = "wasm32"))]
        self.spawn_file_dialog(async {
            let handle = rfd::AsyncFileDialog::new().add_filter("CSV", &["csv"]).pick_file().await?;
            Some(FileDialogAction::OpenOptimizationReport(handle.path().to_owned()))
        });
    }

    /// Show the report in `bytes`, read from the file `name`, in the Results window.
    pub(crate) fn show_optimization_report_file(&mut self, name: String, bytes: &[u8]) -> Result<()> {
        // Its column choices are saved with the scenarios.
        self.ensure_optimization_scenarios_loaded();
        let report = report::read_csv(bytes).map_err(|error| anyhow::anyhow!("{}", tr!("opt-results-open-failed", name = name.clone(), error = format!("{error:#}"))))?;
        userspace_log!("{}", tr!("opt-results-opened", name = name.clone(), count = report.shells.len().to_string()));
        self.editor.optimization.results_view = Some(ResultsView::new(ResultsSource::File { name, report: Arc::new(report) }));
        Ok(())
    }
}

/// A scenario name made safe as a folder or file name: characters no file
/// system takes become `_`, and invisible direction marks (pasted names often
/// carry them) are dropped.
fn folder_name(name: &str) -> String {
    let safe: String = name
        .trim()
        .chars()
        .filter(|character| !matches!(character, '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{FEFF}'))
        .map(|character| {
            if matches!(character, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || character.is_control() {
                '_'
            } else {
                character
            }
        })
        .collect();
    let safe = safe.trim_matches(|character: char| character == '.' || character.is_whitespace()).to_owned();
    if safe.is_empty() { "scenario".to_owned() } else { safe }
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
