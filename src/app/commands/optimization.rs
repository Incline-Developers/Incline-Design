use std::io;

use anyhow::Result;

use crate::{
    app::{App, commands::file::FileDialogAction},
    i18n::tr,
    model::{
        SceneEntityId,
        optimization::{OptimizationScenario, ScenarioFile, unique_name},
    },
    ui::state::{ScenarioDraft, ShellStartPick},
    userspace_log, userspace_warn,
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
        let Some((block_model, marker_z)) = self
            .block_models
            .iter()
            .find(|model| model.name == name && model.state.loaded)
            .map(|model| (model.id, model.world_bounds().map_or(0.0, |(_, max)| max.z)))
        else {
            userspace_warn!("{}", tr!("opt-pick-needs-block-model"));
            return;
        };
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
        });
        self.invalidate_geometry();
        self.invalidate_overlay();
        // Plan view and zoom to all, on what is left showing.
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
