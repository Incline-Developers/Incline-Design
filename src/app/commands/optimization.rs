use std::io;

use anyhow::Result;

use crate::{
    app::App,
    i18n::tr,
    model::optimization::{OptimizationScenario, ScenarioFile, unique_name},
    ui::state::ScenarioDraft,
    userspace_log, userspace_warn,
};

impl<'a> App<'a> {
    /// Show the list. With a scenario open in the editor the list stays behind
    /// it: closing the editor is what asks about unsaved changes.
    pub(crate) fn open_optimization_scenarios(&mut self) {
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
    pub(crate) fn save_optimization_scenario(&mut self, scenario: OptimizationScenario, then_close: bool) -> Result<()> {
        let state = &mut self.editor.optimization;
        match state.scenarios.iter_mut().find(|existing| existing.id == scenario.id) {
            Some(existing) => *existing = scenario.clone(),
            None => state.scenarios.push(scenario.clone()),
        }
        self.write_optimization_scenarios()?;
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
        self.write_optimization_scenarios()
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
        self.write_optimization_scenarios()
    }

    pub(crate) fn rename_optimization_scenario(&mut self, id: u64, name: String) -> Result<()> {
        let name = name.trim().to_owned();
        match self.editor.optimization.scenarios.iter_mut().find(|scenario| scenario.id == id) {
            Some(scenario) if !name.is_empty() && scenario.name != name => scenario.name = name,
            _ => return Ok(()),
        }
        self.write_optimization_scenarios()
    }

    pub(crate) fn load_optimization_scenarios(&mut self) -> Result<()> {
        match crate::app::io::load_optimization_scenarios() {
            Ok(file) => {
                let count = file.scenarios.len();
                self.editor.optimization.scenarios = file.scenarios;
                let kept: std::collections::HashSet<u64> = self.editor.optimization.scenarios.iter().map(|scenario| scenario.id).collect();
                self.editor.optimization.runs.retain(|id, _| kept.contains(id));
                self.editor.optimization.list_open = true;
                userspace_log!("{}", tr!("opt-scenarios-loaded", count = count.to_string()));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                userspace_warn!("{}", tr!("opt-no-scenarios-saved"));
            }
            Err(error) => anyhow::bail!("{}", tr!("opt-scenarios-load-failed", error = error.to_string())),
        }
        Ok(())
    }

    fn write_optimization_scenarios(&self) -> Result<()> {
        let file = ScenarioFile::new(self.editor.optimization.scenarios.clone());
        crate::app::io::save_optimization_scenarios(&file).map_err(|error| anyhow::anyhow!("{}", tr!("opt-scenarios-save-failed", error = error.to_string())))
    }
}
