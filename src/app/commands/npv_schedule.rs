//! Schedules of optimization scenarios: add, edit, save, delete, duplicate,
//! rename, and move or copy one to another scenario. Every change is handed
//! to the project with the scenarios (`write_optimization_scenarios`), so the
//! project reads unsaved until it is saved. See
//! `.claude/specs/scheduling/stage-1-ui.md`. Running one is stage 2
//! (`stage-2-engine.md`): the job is [`npv_schedule_run::run`].

use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    sync::Arc,
};

use anyhow::Result;

use crate::{
    app::{
        App,
        commands::{file::FileDialogAction, optimization::folder_name},
        jobs::JobKey,
    },
    i18n::tr,
    model::{
        block_model::BlockModelId,
        npv_schedule::{NpvSchedule, PeriodCase, PushbackChoice, format_money},
        npv_schedule_run::{self, NpvScheduleInput, NpvScheduleOutcome, NpvScheduleResult, report, simulate::Held},
        optimization::{BlockModelFields, OptimizationScenario, copy_name, unique_name},
    },
    ui::state::{LoadThen, NpvScheduleDraft, NpvScheduleResultsSource, NpvScheduleResultsView, NpvScheduleStatus, NpvScheduleSummary, RunState, ScenarioStatus},
    userspace_error, userspace_log, userspace_warn,
};

/// A name no other schedule of `scenario` has (other than `id` itself).
fn free_name(scenario: &OptimizationScenario, id: u64, wanted: &str) -> String {
    unique_name(
        scenario.schedules.iter().filter(|schedule| schedule.id != id).map(|schedule| schedule.name.as_str()),
        wanted,
    )
}

fn copy_of(scenario: &OptimizationScenario, name: &str) -> String {
    copy_name(scenario.schedules.iter().map(|schedule| schedule.name.as_str()), name, " ", &tr!("opt-copy-suffix"))
}

impl<'a> App<'a> {
    fn scenario_mut(&mut self, id: u64) -> Option<&mut OptimizationScenario> {
        self.editor.optimization.scenarios.iter_mut().find(|scenario| scenario.id == id)
    }

    /// Start a new schedule of `parent` in the schedule editor, hiding the list.
    pub(crate) fn add_npv_schedule(&mut self, parent: u64) {
        let id = self.editor.optimization.fresh_id();
        let Some(scenario) = self.editor.optimization.scenarios.iter().find(|scenario| scenario.id == parent) else {
            return;
        };
        let name = free_name(scenario, id, &tr!("sched-default-name"));
        let mut schedule = NpvSchedule::new(id, name);
        // One row per method and sold element, so the limits start complete.
        schedule.add_all_limits(scenario);
        self.editor.optimization.schedule_draft = Some(NpvScheduleDraft::new(parent, schedule, true));
        self.editor.optimization.list_open = false;
    }

    pub(crate) fn edit_npv_schedule(&mut self, parent: u64, id: u64) {
        let state = &mut self.editor.optimization;
        let Some(schedule) = state
            .scenarios
            .iter()
            .find(|scenario| scenario.id == parent)
            .and_then(|scenario| scenario.schedules.iter().find(|schedule| schedule.id == id))
            .cloned()
        else {
            return;
        };
        state.schedule_draft = Some(NpvScheduleDraft::new(parent, schedule, false));
        state.list_open = false;
    }

    /// Put the edited schedule in its scenario (replacing its earlier copy, or
    /// added at the end) and hand the scenarios to the project. The editor
    /// stays open, holding it as saved, unless closing.
    pub(crate) fn save_npv_schedule(&mut self, parent: u64, mut schedule: NpvSchedule, then_close: bool) -> Result<()> {
        let Some(scenario) = self.scenario_mut(parent) else {
            anyhow::bail!("{}", tr!("sched-parent-gone"));
        };
        schedule.name = free_name(scenario, schedule.id, schedule.name.trim());
        match scenario.schedules.iter_mut().find(|existing| existing.id == schedule.id) {
            // The name is changed in the list only; the editor's copy never wins over it.
            Some(existing) => {
                schedule.name = existing.name.clone();
                *existing = schedule.clone();
            }
            None => scenario.schedules.push(schedule.clone()),
        }
        self.editor.optimization.expanded.insert(parent);
        self.write_optimization_scenarios()?;
        let state = &mut self.editor.optimization;
        if then_close {
            state.schedule_draft = None;
            state.list_open = true;
        } else if let Some(draft) = state.schedule_draft.as_mut().filter(|draft| draft.schedule.id == schedule.id) {
            draft.schedule.name = schedule.name.clone();
            draft.saved = schedule;
            draft.is_new = false;
            draft.confirm_close = false;
        }
        Ok(())
    }

    pub(crate) fn delete_npv_schedule(&mut self, parent: u64, id: u64) -> Result<()> {
        self.cancel_jobs(|key| *key == JobKey::NpvScheduleRun(id));
        if let Some(scenario) = self.scenario_mut(parent) {
            scenario.schedules.retain(|schedule| schedule.id != id);
        }
        let state = &mut self.editor.optimization;
        state.schedule_runs.remove(&id);
        state.schedule_summaries.remove(&id);
        state.schedule_results.remove(&id);
        self.write_optimization_scenarios()
    }

    /// Copy a schedule to a new one right after it. The copy has not been run.
    pub(crate) fn duplicate_npv_schedule(&mut self, parent: u64, id: u64) -> Result<()> {
        let fresh = self.editor.optimization.fresh_id();
        let Some(scenario) = self.scenario_mut(parent) else {
            return Ok(());
        };
        let Some(position) = scenario.schedules.iter().position(|schedule| schedule.id == id) else {
            return Ok(());
        };
        let mut copy = scenario.schedules[position].clone();
        copy.id = fresh;
        copy.name = copy_of(scenario, &copy.name);
        scenario.schedules.insert(position + 1, copy);
        self.write_optimization_scenarios()
    }

    pub(crate) fn rename_npv_schedule(&mut self, parent: u64, id: u64, name: String) -> Result<()> {
        let name = name.trim().to_owned();
        let Some(scenario) = self.scenario_mut(parent) else {
            return Ok(());
        };
        let name = free_name(scenario, id, &name);
        match scenario.schedules.iter_mut().find(|schedule| schedule.id == id) {
            Some(schedule) if !name.is_empty() && schedule.name != name => schedule.name = name.clone(),
            _ => return Ok(()),
        }
        // The open editor shows the new name in its title.
        if let Some(draft) = self.editor.optimization.schedule_draft.as_mut().filter(|draft| draft.schedule.id == id) {
            draft.schedule.name = name.clone();
            draft.saved.name = name;
        }
        self.write_optimization_scenarios()
    }

    /// Move a schedule to scenario `to`, or (`keep`) copy it there. It keeps
    /// every setting, including those `to` has no counterpart for: they show
    /// red there rather than being dropped.
    pub(crate) fn move_npv_schedule(&mut self, id: u64, from: u64, to: u64, keep: bool) -> Result<()> {
        if from == to {
            return Ok(());
        }
        if !keep {
            self.cancel_jobs(|key| *key == JobKey::NpvScheduleRun(id));
        }
        let fresh = self.editor.optimization.fresh_id();
        let Some(source) = self.scenario_mut(from) else {
            return Ok(());
        };
        let Some(position) = source.schedules.iter().position(|schedule| schedule.id == id) else {
            return Ok(());
        };
        let mut schedule = if keep { source.schedules[position].clone() } else { source.schedules.remove(position) };
        let Some(target) = self.scenario_mut(to) else {
            // Put back what was taken: there is nowhere to move it.
            if !keep && let Some(source) = self.scenario_mut(from) {
                source.schedules.insert(position, schedule);
            }
            return Ok(());
        };
        if keep {
            schedule.id = fresh;
        }
        let taken = target.schedules.iter().any(|existing| existing.name.eq_ignore_ascii_case(&schedule.name));
        if taken {
            schedule.name = copy_of(target, &schedule.name);
        }
        let issues = schedule.issues(target).len();
        let (name, scenario) = (schedule.name.clone(), target.name.clone());
        target.schedules.push(schedule);
        if !keep {
            // Its run was of the old scenario's shells.
            let state = &mut self.editor.optimization;
            state.schedule_runs.remove(&id);
            state.schedule_summaries.remove(&id);
            state.schedule_results.remove(&id);
        }
        let message = if keep {
            tr!("sched-copied-to", name = name, scenario = scenario)
        } else {
            tr!("sched-moved-to", name = name, scenario = scenario)
        };
        userspace_log!("{message}");
        if issues > 0 {
            userspace_warn!("{}", tr!("sched-moved-issues", count = issues.to_string()));
        }
        self.write_optimization_scenarios()
    }

    /// Run a schedule on its scenario's run of this session, or stop it while
    /// it runs (the row's play button does both).
    pub(crate) fn run_npv_schedule(&mut self, parent: u64, id: u64) {
        let state = &self.editor.optimization;
        let Some(scenario) = state.scenarios.iter().find(|scenario| scenario.id == parent).cloned() else {
            return;
        };
        let Some(schedule) = scenario.schedules.iter().find(|schedule| schedule.id == id).cloned() else {
            return;
        };
        let status = state.schedule_status(&scenario, &schedule);
        if matches!(status, NpvScheduleStatus::Running(_) | NpvScheduleStatus::Queued) {
            self.cancel_npv_schedule_run(id, &schedule.name);
            return;
        }
        let issues = schedule.issues(&scenario);
        if !issues.is_empty() {
            userspace_warn!("{}", tr!("sched-run-issues", name = schedule.name.clone(), issues = issues.join("; ")));
            return;
        }
        // No up-to-date run of the scenario: run it first, then this.
        if status == NpvScheduleStatus::Blocked {
            let waiting = self.editor.optimization.schedules_after_run.entry(parent).or_default();
            if !waiting.contains(&id) {
                waiting.push(id);
            }
            userspace_log!("{}", tr!("sched-run-parent-first", name = schedule.name.clone(), scenario = scenario.name.clone()));
            let state = &self.editor.optimization;
            if !matches!(state.status(&scenario), ScenarioStatus::Running(_) | ScenarioStatus::Queued) {
                self.run_optimization_scenario(parent);
            }
            return;
        }
        // The block model and topography are read back first, if the user agrees.
        let unloaded = self.unloaded_optimization_inputs(&scenario);
        if !unloaded.is_empty() {
            self.ask_to_load_for_optimization(unloaded, LoadThen::Schedules(vec![(parent, id)]));
            return;
        }
        self.launch_npv_schedule_run(scenario, schedule);
    }

    /// Run every schedule of scenario `parent` that is not running or waiting
    /// already, side by side (each its own job; a scenario without an
    /// up-to-date run is run once first, and they follow it).
    pub(crate) fn run_all_npv_schedules(&mut self, parent: u64) {
        let state = &self.editor.optimization;
        let Some(scenario) = state.scenarios.iter().find(|scenario| scenario.id == parent) else {
            return;
        };
        let idle: Vec<u64> = scenario
            .schedules
            .iter()
            .filter(|schedule| !matches!(state.schedule_status(scenario, schedule), NpvScheduleStatus::Running(_) | NpvScheduleStatus::Queued))
            .map(|schedule| schedule.id)
            .collect();
        userspace_log!("{}", tr!("sched-run-all", scenario = scenario.name.clone(), count = idle.len().to_string()));
        // They would all write the same period field: none does.
        self.editor.optimization.no_period_field.extend(idle.iter().copied());
        for id in idle {
            self.run_npv_schedule(parent, id);
        }
    }

    fn cancel_npv_schedule_run(&mut self, id: u64, name: &str) {
        self.cancel_jobs(|key| *key == JobKey::NpvScheduleRun(id));
        let state = &mut self.editor.optimization;
        state.awaiting_restore.remove(&id);
        state.no_period_field.remove(&id);
        let mut waited = false;
        for waiting in state.schedules_after_run.values_mut() {
            let before = waiting.len();
            waiting.retain(|other| *other != id);
            waited |= waiting.len() != before;
        }
        state.schedules_after_run.retain(|_, waiting| !waiting.is_empty());
        if state.schedule_runs.remove(&id).is_some() || waited {
            userspace_log!("{}", tr!("sched-run-cancelled", name = name.to_owned()));
        }
    }

    /// The scenario's run is over: run the schedules that waited for it, or,
    /// when it did not finish, say they will not run.
    pub(crate) fn after_scenario_run(&mut self, parent: u64, finished: bool) {
        let Some(waiting) = self.editor.optimization.schedules_after_run.remove(&parent) else {
            return;
        };
        if !finished {
            userspace_warn!("{}", tr!("sched-run-parent-failed", count = waiting.len().to_string()));
            return;
        }
        for id in waiting {
            self.run_npv_schedule(parent, id);
        }
    }

    fn fail_npv_schedule_run(&mut self, id: u64, name: &str, issues: &[String]) {
        userspace_error!("{}", tr!("sched-run-cannot-start", name = name.to_owned()));
        for issue in issues {
            userspace_error!("{}", tr!("opt-run-issue", issue = issue.clone()));
        }
        self.editor.optimization.schedule_runs.remove(&id);
        self.editor.optimization.awaiting_restore.remove(&id);
    }

    /// Everything the schedule needs is in memory: prepare its scenario again
    /// and spawn the job.
    fn launch_npv_schedule_run(&mut self, scenario: OptimizationScenario, schedule: NpvSchedule) {
        let id = schedule.id;
        self.editor.optimization.awaiting_restore.remove(&id);
        let Some(run) = self.editor.optimization.results.get(&scenario.id).cloned() else {
            self.fail_npv_schedule_run(id, &schedule.name, &[tr!("sched-blocked")]);
            return;
        };
        let (prepared, layout, model_id) = match self.prepare_scenario_inputs(&scenario) {
            Ok(inputs) => inputs,
            Err(issues) => {
                self.fail_npv_schedule_run(id, &schedule.name, &issues);
                return;
            }
        };
        // Checked before the run, so a field that cannot be written does not
        // cost the run's time first.
        let fields = self
            .block_models
            .iter()
            .find(|model| model.id == model_id)
            .map(BlockModelFields::of_open)
            .unwrap_or_default();
        // Started by Run all: no period field, only the kept mining order.
        let period_field = if self.editor.optimization.no_period_field.remove(&id) {
            None
        } else {
            match schedule.period_field_target(&scenario, &fields) {
                Ok(field) => field,
                Err(issue) => {
                    self.fail_npv_schedule_run(id, &schedule.name, &[issue]);
                    return;
                }
            }
        };
        let input = NpvScheduleInput {
            scenario_name: scenario.name.clone(),
            schedule_name: schedule.name.clone(),
            prepared,
            layout,
            run,
            settings: schedule.resolve(&scenario),
            case: schedule.period_case,
            write_field: period_field.is_some(),
        };
        userspace_log!(
            "{}",
            tr!(
                "sched-run-started",
                name = schedule.name.clone(),
                pits = match schedule.pushbacks {
                    PushbackChoice::EveryShell => input.run.shells.summaries.len(),
                    PushbackChoice::Factors => schedule.pushback_factors.len(),
                }
                .to_string()
            )
        );
        let fingerprint = self.editor.optimization.schedule_fingerprint(&scenario, &schedule);
        let started = web_time::Instant::now();
        let progress = self.spawn_job_sharing_progress(
            tr!("sched-run-label", name = schedule.name.clone()),
            vec![JobKey::NpvScheduleRun(id), JobKey::BlockModel(model_id)],
            move |cancel, progress| npv_schedule_run::run(input, cancel, progress),
            move |app, result| app.finish_npv_schedule_run(&schedule, fingerprint, started, model_id, period_field, result),
        );
        self.editor.optimization.schedule_runs.insert(id, RunState::Running { progress });
    }

    fn finish_npv_schedule_run(
        &mut self,
        schedule: &NpvSchedule,
        fingerprint: u64,
        started: web_time::Instant,
        model_id: BlockModelId,
        period_field: Option<String>,
        result: Result<NpvScheduleOutcome>,
    ) {
        let id = schedule.id;
        let name = schedule.name.clone();
        // Deleted, moved or cancelled meanwhile: the result is of nothing shown.
        if !matches!(self.editor.optimization.schedule_runs.get(&id), Some(RunState::Running { .. })) {
            return;
        }
        let outcome = match result {
            Ok(outcome) => outcome,
            Err(error) => {
                self.editor.optimization.schedule_runs.remove(&id);
                if format!("{error:#}").contains("Cancelled") {
                    userspace_log!("{}", tr!("sched-run-cancelled", name = name));
                } else {
                    userspace_error!("{}", tr!("sched-run-failed", name = name, error = format!("{error:#}")));
                }
                return;
            }
        };
        let NpvScheduleOutcome { result, field } = outcome;
        let state = &mut self.editor.optimization;
        state.schedule_runs.insert(id, RunState::Finished { fingerprint });
        userspace_log!(
            "{}",
            tr!("sched-run-finished", name = name.clone(), seconds = format!("{:.1}", started.elapsed().as_secs_f64()))
        );
        // The schedule: every pushback, fixed lead.
        match result.schedule() {
            Some(pit) => {
                let schedule_pit = &pit.lead;
                let peak = result.peak(PeriodCase::FixedLead).filter(|&peak| peak + 1 < result.pits.len());
                let summary = NpvScheduleSummary {
                    periods: schedule_pit.periods.len(),
                    peak_before_last: peak.map(|peak| peak + 1),
                    npv_best: pit.best.npv,
                    npv_worst: pit.worst.npv,
                    npv_lead: schedule_pit.npv,
                };
                let (lead, best, worst) = (format_money(summary.npv_lead), format_money(summary.npv_best), format_money(summary.npv_worst));
                userspace_log!(
                    "{}",
                    tr!(
                        "sched-run-schedule",
                        name = name.clone(),
                        pushbacks = result.pits.len().to_string(),
                        periods = summary.periods.to_string(),
                        npv = lead.clone(),
                        best = best.clone(),
                        worst = worst.clone()
                    )
                );
                if schedule_pit.unfinished {
                    userspace_warn!("{}", tr!("sched-run-unfinished-schedule", name = name.clone(), max = schedule.max_periods.to_string()));
                }
                if let Some(peak) = peak {
                    userspace_log!(
                        "{}",
                        tr!(
                            "sched-run-peak-advice",
                            name = name.clone(),
                            pushback = (peak + 1).to_string(),
                            npv = format_money(result.pits[peak].lead.npv)
                        )
                    );
                }
                // Fixed lead should fall between the bounds.
                let (low, high) = (summary.npv_worst.min(summary.npv_best), summary.npv_worst.max(summary.npv_best));
                let slack = 1e-6 * high.abs().max(1.0);
                if summary.npv_lead < low - slack || summary.npv_lead > high + slack {
                    userspace_warn!("{}", tr!("sched-run-outside-bounds", name = name.clone(), lead = lead, best = best, worst = worst));
                }
                // Where mining waited because ore had nowhere to go.
                for reason in [Held::NoStockpile, Held::StockpilesFull] {
                    let periods: Vec<String> = schedule_pit
                        .periods
                        .iter()
                        .enumerate()
                        .filter(|(_, period)| period.held == Some(reason))
                        .map(|(index, _)| (index + 1).to_string())
                        .collect();
                    if !periods.is_empty() {
                        let periods = periods.join(", ");
                        let message = match reason {
                            Held::NoStockpile => tr!("sched-run-held-no-stockpile", periods = periods),
                            Held::StockpilesFull => tr!("sched-run-held-stockpiles-full", periods = periods),
                        };
                        userspace_warn!("{message}");
                    }
                }
                self.editor.optimization.schedule_summaries.insert(id, summary);
            }
            None => {
                userspace_warn!("{}", tr!("sched-run-no-pushbacks", name = name.clone()));
                self.editor.optimization.schedule_summaries.remove(&id);
            }
        }
        if let Some(order) = result.order.as_ref().filter(|_| period_field.is_some()) {
            userspace_log!("{}", tr!("sched-run-field-pit", case = order.case.label()));
        }
        if schedule.create_reports {
            self.write_npv_schedule_reports(&result);
        }
        self.editor.optimization.schedule_results.insert(id, Arc::new(result));
        if let (Some(field), Some(values)) = (period_field, field) {
            let seed = {
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                (id, started.elapsed().as_nanos()).hash(&mut hasher);
                hasher.finish()
            };
            self.write_shell_field(model_id, field, values, HashMap::new(), seed);
        }
    }

    /// A finished run's CSV files, into its scenario's reports folder (or
    /// downloaded in the browser).
    fn write_npv_schedule_reports(&mut self, result: &NpvScheduleResult) {
        let Some(scenario) = self.editor.optimization.scenarios.iter().find(|scenario| scenario.name == result.scenario).cloned() else {
            return;
        };
        if cfg!(not(target_arch = "wasm32")) && scenario.output.reports_folder.trim().is_empty() {
            userspace_warn!("{}", tr!("sched-res-no-reports-folder", name = result.name.clone()));
            return;
        }
        let stem = folder_name(&result.name);
        for (ending, bytes) in [
            (report::PERIODS_FILE, report::write_periods_csv(result)),
            (report::PITS_FILE, report::write_pits_csv(result)),
        ] {
            match bytes {
                Ok(bytes) => self.write_scenario_report_file(&scenario, &format!("{stem}_{ending}"), bytes),
                Err(error) => userspace_error!("{}", tr!("sched-res-export-failed", error = format!("{error:#}"))),
            }
        }
    }

    /// Open the schedule Results window on a schedule's last run.
    pub(crate) fn open_npv_schedule_results(&mut self, parent: u64, id: u64) {
        if !self.editor.optimization.schedule_results.contains_key(&id) {
            return;
        }
        self.editor.optimization.schedule_results_view = Some(NpvScheduleResultsView::new(NpvScheduleResultsSource::Run { parent, id }));
    }

    /// Save what the schedule Results window shows as its two CSV files: the
    /// periods file where the user picks, the pits file beside it (two
    /// downloads in the browser).
    pub(crate) fn export_npv_schedule_report(&mut self) {
        let Some(result) = self.editor.optimization.schedule_results_result() else {
            return;
        };
        let (periods, pits) = match (report::write_periods_csv(&result), report::write_pits_csv(&result)) {
            (Ok(periods), Ok(pits)) => (periods, pits),
            (Err(error), _) | (_, Err(error)) => {
                userspace_error!("{}", tr!("sched-res-export-failed", error = format!("{error:#}")));
                return;
            }
        };
        let stem = folder_name(&result.name);
        #[cfg(target_arch = "wasm32")]
        {
            Self::trigger_browser_download(format!("{stem}_{}", report::PERIODS_FILE), periods, "text/csv", tr!("err-file-desc-sched-report"));
            Self::trigger_browser_download(format!("{stem}_{}", report::PITS_FILE), pits, "text/csv", tr!("err-file-desc-sched-report"));
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let file_name = format!("{stem}_{}", report::PERIODS_FILE);
            self.spawn_file_dialog(async move {
                let handle = rfd::AsyncFileDialog::new().add_filter("CSV", &["csv"]).set_file_name(file_name).save_file().await?;
                Some(FileDialogAction::ExportNpvScheduleReport {
                    path: handle.path().to_owned(),
                    periods,
                    pits,
                })
            });
        }
    }

    /// Write the exported files: the periods file at `path`, the pits file
    /// beside it (`_periods` in the name becomes `_pits`, or `_pits` is added).
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn write_npv_schedule_report_export(&mut self, mut path: std::path::PathBuf, periods: Vec<u8>, pits: Vec<u8>) -> Result<()> {
        if path.extension().is_none() {
            path.set_extension("csv");
        }
        let stem = path.file_stem().map(|stem| stem.to_string_lossy().into_owned()).unwrap_or_default();
        let pits_stem = match stem.strip_suffix("_periods") {
            Some(base) => format!("{base}_pits"),
            None => format!("{stem}_pits"),
        };
        let pits_path = path.with_file_name(format!("{pits_stem}.csv"));
        for (path, bytes) in [(&path, &periods), (&pits_path, &pits)] {
            crate::model::atomic_file::write_atomic(path, |file| Ok(std::io::Write::write_all(file, bytes)?))
                .map_err(|error| anyhow::anyhow!("{}", tr!("sched-res-export-failed", error = format!("{error:#}"))))?;
            userspace_log!("{}", tr!("opt-results-exported", path = path.display().to_string()));
        }
        Ok(())
    }

    /// Ask for a schedule periods CSV and show it in the Results window.
    pub(crate) fn open_npv_schedule_report(&mut self) {
        #[cfg(target_arch = "wasm32")]
        self.spawn_file_dialog(async {
            let handle = rfd::AsyncFileDialog::new().add_filter("CSV", &["csv"]).pick_file().await?;
            Some(FileDialogAction::WebOpenScheduleReport(crate::model::input::read_browser_handle(handle).await))
        });
        #[cfg(not(target_arch = "wasm32"))]
        self.spawn_file_dialog(async {
            let handle = rfd::AsyncFileDialog::new().add_filter("CSV", &["csv"]).pick_file().await?;
            Some(FileDialogAction::OpenNpvScheduleReport(handle.path().to_owned()))
        });
    }

    /// Show the schedule periods file in `bytes`, read from file `name`.
    pub(crate) fn show_npv_schedule_report_file(&mut self, name: String, bytes: &[u8]) -> Result<()> {
        // Its column choices are saved with the scenarios.
        self.ensure_optimization_scenarios_loaded();
        let result = report::read_periods_csv(bytes).map_err(|error| anyhow::anyhow!("{}", tr!("sched-res-open-failed", name = name.clone(), error = format!("{error:#}"))))?;
        userspace_log!("{}", tr!("sched-res-opened", name = name.clone(), count = result.pits.len().to_string()));
        self.editor.optimization.schedule_results_view = Some(NpvScheduleResultsView::new(NpvScheduleResultsSource::File { name, result: Arc::new(result) }));
        Ok(())
    }

    /// A copy of schedule `id` with the shells of `factors` as its chosen
    /// pushbacks, saved and opened in the editor.
    pub(crate) fn npv_schedule_from_pushbacks(&mut self, parent: u64, id: u64, mut factors: Vec<f64>) -> Result<()> {
        if factors.is_empty() {
            return Ok(());
        }
        factors.sort_by(f64::total_cmp);
        factors.dedup();
        let fresh = self.editor.optimization.fresh_id();
        let Some(scenario) = self.scenario_mut(parent) else {
            return Ok(());
        };
        let Some(source) = scenario.schedules.iter().find(|schedule| schedule.id == id) else {
            return Ok(());
        };
        let mut copy = source.clone();
        copy.id = fresh;
        copy.name = copy_of(scenario, &copy.name);
        copy.pushbacks = PushbackChoice::Factors;
        copy.pushback_factors = factors;
        let name = copy.name.clone();
        scenario.schedules.push(copy);
        self.editor.optimization.expanded.insert(parent);
        userspace_log!("{}", tr!("sched-res-new-schedule", name = name));
        self.write_optimization_scenarios()?;
        self.editor.optimization.schedule_results_view = None;
        self.edit_npv_schedule(parent, fresh);
        Ok(())
    }
}
