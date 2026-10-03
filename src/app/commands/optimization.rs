use std::io;

use anyhow::Result;

use crate::{
    app::{App, commands::file::FileDialogAction},
    i18n::tr,
    model::optimization::{OptimizationScenario, ScenarioFile, unique_name},
    ui::state::ScenarioDraft,
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
}
