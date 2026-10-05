//! Shared layout for the planning Setup subpages.
//!
//! Solids' Setup is the Reserves setup: a project-wide Field List (summed,
//! weighted-average, or category columns, e.g. Tonnes, Fe, and Rock Type)
//! and, per block model opted in via its own checkbox, a mapping of that
//! model's own columns or constants onto the list, with the resulting
//! totals.
//!
//! Schedule's and Haulage's Setup steps live in [`super::schedule_setup`]
//! and their own modules; this one supplies the step lists, run controls and
//! the panes they are arranged in. The grids are the reusable
//! [`data_grid`](crate::ui::widgets::data_grid) widgets.
use thousands::Separable;

use crate::{
    i18n::tr,
    model::{
        BenchInterval, BenchingPlan, Document, ReserveAggregation, ReserveFieldId, SolidEdit, SolidKind,
        block_model::{OpenBlockModel, ReserveMappingSource},
    },
    ui::{
        EditorState, UiProjectView, chrome,
        dialogs::solids::{block_model_label, block_model_options, kind_label, triangulation_label, triangulation_options},
        elements::solids_view::format_rl,
        fonts::bold,
        state::{PlanningPage, SolidsStep, UiCommand},
        unthemed_icon,
        widgets::{
            context_menu::{ContextMenuAction, context_menu_popup},
            data_grid::{
                CELL_WARNING_WIDTH, CellOption, DataGrid, GridRow, grid_add_action_row, grid_add_row, grid_cell_color, grid_cell_combo, grid_cell_fixed, grid_cell_number,
                grid_cell_warning, grid_columns_row, grid_empty_state, grid_group_row, grid_row, grid_separator_row, property_table_height,
            },
            explorer::{ExplorerEntry, paint_fixed_stripes, reserve_fixed_stripes},
            island::{Island, Side},
        },
    },
};

fn striped_list(ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui)) {
    ui.set_clip_rect(ui.clip_rect().intersect(ui.max_rect()));
    egui::ScrollArea::vertical().auto_shrink([false; 2]).min_scrolled_height(0.0).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
        let (slot, top) = reserve_fixed_stripes(ui);
        content(ui);
        paint_fixed_stripes(ui, slot, top, crate::ui::widgets::tree_row_colors(ui).1);
    });
}

pub(crate) fn draw_steps(ui: &mut egui::Ui, editor: &mut EditorState, page: PlanningPage, commands: &mut Vec<UiCommand>) {
    match page {
        PlanningPage::Solids => draw_solids_steps(ui, editor, commands),
        PlanningPage::Schedule => striped_list(ui, |ui| super::schedule_setup::draw_steps(ui, editor, commands)),
        PlanningPage::Haulage => striped_list(ui, |ui| draw_haulage_steps(ui, editor, commands)),
    }
}

/// Schedule's Haulage step: where each Haulage step stands, each opening its
/// own page. The steps are set up there; this pipeline only waits on them.
fn draw_haulage_summary(ui: &mut egui::Ui, rect: egui::Rect, editor: &mut EditorState) {
    use crate::ui::{state::HaulageStep, widgets::data_grid::grid_columns_row};
    const FRACTIONS: [f32; 2] = [0.55, 0.45];
    let columns = [(tr!("planning-step"), FRACTIONS[0]), (tr!("planning-status"), FRACTIONS[1])];
    DataGrid::new("schedule_haulage_steps", rect, &tr!("planning-page-haulage"))
        .columns(&columns)
        .show(ui, |ui| {
            for step in HaulageStep::ALL {
                let state = editor.haulage_stages[step.index()].state.label();
                let (response, _) = grid_columns_row(ui, &FRACTIONS, &[&step.label(), &state], false);
                if response.on_hover_text(tr!("schedule-haulage-open")).clicked() {
                    editor.planning_page = crate::ui::state::PlanningPage::Haulage;
                    editor.haulage_subpage = crate::ui::state::PlanningSubpage::Setup;
                    editor.haulage_setup_step = step;
                }
            }
        });
}

/// Open the Haulage page's Layout, where a connection is fixed.
fn open_haul_layout(editor: &mut EditorState) {
    editor.planning_page = PlanningPage::Haulage;
    editor.haulage_subpage = crate::ui::state::PlanningSubpage::Layout;
}

/// Each destination's dump point, and each stockpile's reclaim point, and
/// whether that reaches the roads. A row opens the Layout to fix it.
fn draw_destination_connections(ui: &mut egui::Ui, rect: egui::Rect, editor: &mut EditorState) {
    use crate::{
        model::schedule::DestinationKind,
        ui::widgets::data_grid::{grid_cell_warning, grid_columns_row, grid_group_row},
    };
    const FRACTIONS: [f32; 3] = [0.3, 0.35, 0.35];
    let columns = [
        (tr!("planning-name"), FRACTIONS[0]),
        (tr!("haul-link-dump-column"), FRACTIONS[1]),
        (tr!("haul-link-reclaim-column"), FRACTIONS[2]),
    ];
    let connections = editor.schedule_haul_connections.clone();
    let mut open = false;
    DataGrid::new("schedule_haul_connections", rect, &tr!("haul-connections")).columns(&columns).show(ui, |ui| {
        let Some(connections) = connections else {
            grid_empty_state(ui, &tr!("haul-connections-not-run"), None);
            return;
        };
        if connections.destinations.is_empty() {
            grid_empty_state(ui, &tr!("haul-connections-no-destinations"), None);
            return;
        }
        for kind in [DestinationKind::Stockpile, DestinationKind::Dump, DestinationKind::Crusher] {
            let entries: Vec<_> = connections.destinations.iter().filter(|entry| entry.kind == kind).collect();
            if entries.is_empty() {
                continue;
            }
            let heading = match kind {
                DestinationKind::Stockpile => tr!("planning-stockpiles"),
                DestinationKind::Dump => tr!("planning-dumps"),
                DestinationKind::Crusher => tr!("destination-crushers"),
            };
            if !grid_group_row(ui, ("schedule_haul_kind", kind as u8), &heading, &entries.len().to_string(), 0) {
                continue;
            }
            for (index, entry) in entries.into_iter().enumerate() {
                let dump = entry.dump.label();
                let reclaim = entry.reclaim.map(|link| link.label()).unwrap_or_default();
                let (response, cells) = grid_columns_row(ui, &FRACTIONS, &[&entry.name, &dump, &reclaim], false);
                for (column, link) in [(1, Some(entry.dump)), (2, entry.reclaim)] {
                    if let Some(link) = link.filter(|link| link.is_problem()) {
                        grid_cell_warning(ui, ("schedule_haul_warning", kind as u8, index, column), cells[column], &link.label());
                    }
                }
                open |= response.on_hover_text(tr!("haul-connections-open")).clicked();
            }
        }
    });
    if open {
        open_haul_layout(editor);
    }
}

/// How many of each pit's dig blocks reach the roads.
fn draw_pit_connections(ui: &mut egui::Ui, rect: egui::Rect, editor: &mut EditorState) {
    use crate::ui::widgets::data_grid::{grid_cell_warning, grid_columns_row};
    const FRACTIONS: [f32; 2] = [0.3, 0.7];
    let columns = [(tr!("haul-link-pit-column"), FRACTIONS[0]), (tr!("haul-link-blocks-column"), FRACTIONS[1])];
    let pits = editor.schedule_haul_connections.as_ref().map(|connections| connections.pits.clone());
    let mut open = false;
    DataGrid::new("schedule_haul_pits", rect, &tr!("haul-connections-pits")).columns(&columns).show(ui, |ui| {
        let Some(pits) = pits else {
            grid_empty_state(ui, &tr!("haul-connections-not-run"), None);
            return;
        };
        if pits.is_empty() {
            grid_empty_state(ui, &tr!("haul-connections-no-blocks"), None);
            return;
        }
        for (index, pit) in pits.iter().enumerate() {
            use thousands::Separable;
            let reached = tr!("haul-link-blocks", reached = pit.reached.separate_with_commas(), total = pit.total.separate_with_commas());
            let (response, cells) = grid_columns_row(ui, &FRACTIONS, &[&pit.name, &reached], false);
            if pit.reached < pit.total {
                let missed = tr!("haul-link-pit-problem", missed = (pit.total - pit.reached).to_string(), total = pit.total.to_string());
                grid_cell_warning(ui, ("schedule_haul_pit_warning", index), cells[1], &missed);
            }
            open |= response.on_hover_text(tr!("haul-connections-open")).clicked();
        }
    });
    if open {
        open_haul_layout(editor);
    }
}

/// The Schedule's Solids step: where each Solids step stands, each row
/// opening that step.
fn draw_solids_summary(ui: &mut egui::Ui, rect: egui::Rect, editor: &mut EditorState) {
    const FRACTIONS: [f32; 2] = [0.55, 0.45];
    let columns = [(tr!("planning-step"), FRACTIONS[0]), (tr!("planning-status"), FRACTIONS[1])];
    DataGrid::new("schedule_solids_steps", rect, &tr!("planning-page-solids")).columns(&columns).show(ui, |ui| {
        for step in SolidsStep::ALL {
            let state = editor.planning_stages[step.index()].state.label();
            let (response, _) = grid_columns_row(ui, &FRACTIONS, &[&step.label(), &state], false);
            if response.on_hover_text(tr!("schedule-solids-open")).clicked() {
                editor.planning_page = PlanningPage::Solids;
                editor.solids_subpage = crate::ui::state::PlanningSubpage::Setup;
                editor.planning_solids_step = step;
            }
        }
    });
}

/// Haulage Setup's steps, marked from the Haulage pipeline.
fn draw_haulage_steps(ui: &mut egui::Ui, editor: &mut EditorState, commands: &mut Vec<UiCommand>) {
    use crate::ui::state::HaulageStep;
    let mut step = editor.haulage_setup_step;
    let mut markers = Vec::with_capacity(HaulageStep::ALL.len());
    for entry in HaulageStep::ALL {
        let status = &editor.haulage_stages[entry.index()];
        ui.horizontal(|ui| {
            ui.add_space(ui.spacing().indent);
            // Truck Classes with no classes is grey, not amber: its note says
            // what is missing, and nothing has gone wrong. Road Network always
            // has its settings, and no roads is fine - destinations then haul
            // their fixed distances.
            let empty = entry == HaulageStep::TruckClasses
                && status.state == crate::app::planning_pipeline::StageState::Complete
                && status.last_success.as_ref().is_some_and(|run| run.entities == 0);
            let badge = if empty { StepBadge::NotRun } else { StepBadge::of(status.state, &status.diagnostics) };
            let entry_response = ExplorerEntry::new(egui::Id::new(("haulage_step", entry as u8)), bold(&entry.label()))
                .leading_icon(badge.icon(), egui::Color32::WHITE)
                .header_aligned_icon()
                .selected(step == entry)
                .show(ui);
            if let Some(rect) = entry_response.icon_rect {
                markers.push((rect, if empty { crate::app::planning_pipeline::StageState::NotRun } else { status.state }));
            }
            let response = entry_response.response.on_hover_ui(|ui| {
                stage_tooltip_parts(ui, status.state, status.blocked_by.map(HaulageStep::label), status.message.as_deref(), &status.diagnostics);
            });
            if response.clicked() {
                step = entry;
            }
            draw_stage_menu(
                &response,
                entry.label(),
                editor.haulage_run_active,
                [UiCommand::RunHaulageStage(entry), UiCommand::RunAllHaulageStages, UiCommand::CancelHaulageRun],
                commands,
            );
        });
    }
    paint_step_links(ui, &markers);
    editor.haulage_setup_step = step;
}

fn draw_solids_steps(ui: &mut egui::Ui, editor: &mut EditorState, commands: &mut Vec<UiCommand>) {
    let mut step = editor.planning_solids_step;
    striped_list(ui, |ui| {
        let mut markers = Vec::with_capacity(SolidsStep::ALL.len());
        for entry in SolidsStep::ALL {
            let status = &editor.planning_stages[entry.index()];
            ui.horizontal(|ui| {
                ui.add_space(ui.spacing().indent);
                let entry_response = ExplorerEntry::new(egui::Id::new(entry.tree_id()), bold(&entry.label()))
                    .leading_icon(StepBadge::of(status.state, &status.diagnostics).icon(), egui::Color32::WHITE)
                    .header_aligned_icon()
                    .selected(step == entry)
                    .show(ui);
                if let Some(rect) = entry_response.icon_rect {
                    markers.push((rect, status.state));
                }
                // The badge's reason, where the badge is.
                let response = entry_response.response.on_hover_ui(|ui| stage_tooltip(ui, status));
                if response.clicked() {
                    step = entry;
                }
                draw_stage_menu(
                    &response,
                    entry.label(),
                    editor.planning_run_active,
                    [UiCommand::RunPlanningStage(entry), UiCommand::RunAllPlanningStages, UiCommand::CancelPlanningRun],
                    commands,
                );
            });
        }
        paint_step_links(ui, &markers);
    });
    editor.planning_solids_step = step;
}

/// Join consecutive step badges with a line showing how far a run reached.
///
/// Painted after all the rows so their backgrounds cannot cover it. Green
/// joins complete steps; a failed step's red line reaches the step after it,
/// which it blocks, and stops there.
pub(crate) fn paint_step_links(ui: &egui::Ui, markers: &[(egui::Rect, crate::app::planning_pipeline::StageState)]) {
    use crate::app::planning_pipeline::StageState;

    let mut failed = false;
    for pair in markers.windows(2) {
        let (previous, state) = pair[0];
        let (next, _) = pair[1];
        failed |= state == StageState::Failed;
        if !matches!(state, StageState::Complete | StageState::Failed) {
            break;
        }
        let color = if failed {
            egui::Color32::from_rgb(0xDC, 0x45, 0x45)
        } else {
            egui::Color32::from_rgb(0x2E, 0xAD, 0x62)
        };
        // The circular badges occupy 12.2 px inside their 16 px SVGs.
        let start = previous.center() + egui::vec2(0.0, 6.1);
        let end = next.center() - egui::vec2(0.0, 6.1);
        if end.y > start.y {
            ui.painter().line_segment([start, end], egui::Stroke::new(2.0, color));
        }
    }
}

/// Run Step and Run All: a light muted green, filled for the one and outlined
/// for the other.
pub(crate) const RUN_STEP_TINT: egui::Color32 = egui::Color32::from_rgb(0x76, 0xC3, 0x8D);
/// Run All: the same green as Run - the pair is one control, told apart by
/// the filled head against the outlined pair rather than by shade.
pub(crate) const RUN_ALL_TINT: egui::Color32 = RUN_STEP_TINT;
/// Cancel: a soft red, muted well below the error red the failed stages carry
/// so it is a button rather than an alarm.
pub(crate) const CANCEL_TINT: egui::Color32 = egui::Color32::from_rgb(0xCB, 0x63, 0x63);

/// What a run bar's buttons asked for this frame.
enum RunAction {
    Step,
    All,
    Cancel,
}

/// A pipeline's run buttons. Idle, Run Step (where the page has a step to run)
/// and Run All; running, Stop alone in their place, so the one control that
/// can act is the only one shown rather than sitting beside two dimmed ones.
fn draw_run_buttons(ui: &mut egui::Ui, salt: &'static str, running: bool, run_step: bool) -> Option<RunAction> {
    use crate::ui::widgets::toolbar::ToolbarButton;

    let side = ui.available_height();
    let button = |icon: egui::ImageSource<'static>, tint: egui::Color32, tooltip: String, id: &'static str| {
        ToolbarButton::new(egui::Image::new(icon).tint(tint), tooltip).button_side(side).id_salt((salt, id))
    };
    if running {
        return ui
            .add(button(unthemed_icon!("stop.svg"), CANCEL_TINT, tr!("stage-cancel"), "cancel"))
            .clicked()
            .then_some(RunAction::Cancel);
    }
    let mut action = None;
    if run_step && ui.add(button(unthemed_icon!("play.svg"), RUN_STEP_TINT, tr!("stage-run-step"), "step")).clicked() {
        action = Some(RunAction::Step);
    }
    if ui.add(button(unthemed_icon!("play_all.svg"), RUN_ALL_TINT, tr!("stage-run-all"), "all")).clicked() {
        action = Some(RunAction::All);
    }
    action
}

/// One pipeline's run controls: its buttons, Auto where the pipeline has it,
/// and its progress, with `hover` saying where it stands.
fn run_header(
    ui: &mut egui::Ui,
    salt: &'static str,
    running: bool,
    run_step: bool,
    auto: Option<(&mut bool, String)>,
    (done, total): (usize, usize),
    hover: impl FnOnce(&mut egui::Ui),
) -> Option<RunAction> {
    ui.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let action = draw_run_buttons(ui, salt, running, run_step);
        if let Some((auto, note)) = auto {
            ui.add_space(10.0);
            ui.add(crate::ui::widgets::toggle::Toggle::new(auto, tr!("planning-auto"))).on_hover_text(note);
        }
        let label = tr!("stage-progress-short", done = done, total = total);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            crate::ui::widgets::progress::draw_planning_progress(ui, &label, done as f32 / total as f32);
        })
        .response
        .on_hover_ui(hover);
        action
    })
    .inner
}

/// Contents of the separate run-control island at the top of the sidebar.
pub(crate) fn draw_solids_run_controls(ui: &mut egui::Ui, editor: &mut EditorState, commands: &mut Vec<UiCommand>) {
    use crate::app::planning_pipeline::StageState;

    let step = editor.planning_solids_step;
    let completed = editor.planning_stages.iter().filter(|stage| stage.state == StageState::Complete).count();
    let active = SolidsStep::ALL.into_iter().find(|step| editor.planning_stages[step.index()].state == StageState::Running);
    let reported = active.unwrap_or(if editor.is_solids_view() { SolidsStep::DigStrips } else { step });
    let status = editor.planning_stages[reported.index()].clone();
    let snapshot = editor.planning_snapshot_status.clone();
    let action = run_header(
        ui,
        "planning_run",
        editor.planning_run_active,
        !editor.is_solids_view(),
        Some((&mut editor.planning_auto_run, tr!("planning-auto-note"))),
        (completed, SolidsStep::ALL.len()),
        |ui| {
            ui.label(reported.label());
            stage_tooltip(ui, &status);
            if !snapshot.is_empty() {
                ui.separator();
                ui.label(&snapshot);
            }
        },
    );
    match action {
        Some(RunAction::Step) => commands.push(UiCommand::RunPlanningStage(step)),
        Some(RunAction::All) => commands.push(UiCommand::RunAllPlanningStages),
        Some(RunAction::Cancel) => commands.push(UiCommand::CancelPlanningRun),
        None => {}
    }
}

/// The Schedule Setup page's run controls: the same buttons and the same
/// progress readout as the Solids page, over its own pipeline.
///
/// Kept beside the Solids controls rather than merged with them: the two
/// pipelines run different things, and one control that switched which
/// pipeline it drove on a page change would be one Cancel that could stop the
/// wrong run.
///
/// Its Auto is the Gantt's: one switch that reruns these steps and
/// recalculates the schedule, shown on both pages.
pub(crate) fn draw_schedule_run_controls(ui: &mut egui::Ui, editor: &mut EditorState, commands: &mut Vec<UiCommand>) {
    use crate::{app::planning_pipeline::StageState, ui::state::ScheduleStep};

    let step = editor.schedule_setup_step;
    let completed = editor.schedule_stages.iter().filter(|stage| stage.state == StageState::Complete).count();
    let active = ScheduleStep::ALL.into_iter().find(|step| editor.schedule_stages[step.index()].state == StageState::Running);
    let reported = active.unwrap_or(step);
    let status = editor.schedule_stages[reported.index()].clone();
    let calculation = editor.schedule_calculation_status.clone();
    let action = run_header(
        ui,
        "schedule_run",
        editor.schedule_run_active,
        true,
        Some((&mut editor.schedule_auto_recalculate, tr!("schedule-setup-auto-note"))),
        (completed, ScheduleStep::ALL.len()),
        |ui| {
            ui.label(reported.label());
            stage_tooltip_parts(ui, status.state, status.blocked_by.map(ScheduleStep::label), status.message.as_deref(), &status.diagnostics);
            // What the Gantt would be told if it asked to calculate now,
            // where the buttons that change that answer are.
            if !calculation.is_empty() {
                ui.separator();
                ui.label(&calculation);
            }
        },
    );
    match action {
        Some(RunAction::Step) => commands.push(UiCommand::RunScheduleStage(step)),
        Some(RunAction::All) => commands.push(UiCommand::RunAllScheduleStages),
        Some(RunAction::Cancel) => commands.push(UiCommand::CancelScheduleRun),
        None => {}
    }
}

/// The Haulage Setup page's run controls, over the Haulage pipeline.
pub(crate) fn draw_haulage_run_controls(ui: &mut egui::Ui, editor: &mut EditorState, commands: &mut Vec<UiCommand>) {
    use crate::{app::planning_pipeline::StageState, ui::state::HaulageStep};

    let step = editor.haulage_setup_step;
    let completed = editor.haulage_stages.iter().filter(|stage| stage.state == StageState::Complete).count();
    let active = HaulageStep::ALL.into_iter().find(|step| editor.haulage_stages[step.index()].state == StageState::Running);
    let reported = active.unwrap_or(step);
    let status = editor.haulage_stages[reported.index()].clone();
    let action = run_header(
        ui,
        "haulage_run",
        editor.haulage_run_active,
        true,
        Some((&mut editor.haulage_auto_run, tr!("planning-auto-note"))),
        (completed, HaulageStep::ALL.len()),
        |ui| {
            ui.label(reported.label());
            stage_tooltip_parts(ui, status.state, status.blocked_by.map(HaulageStep::label), status.message.as_deref(), &status.diagnostics);
        },
    );
    match action {
        Some(RunAction::Step) => commands.push(UiCommand::RunHaulageStage(step)),
        Some(RunAction::All) => commands.push(UiCommand::RunAllHaulageStages),
        Some(RunAction::Cancel) => commands.push(UiCommand::CancelHaulageRun),
        None => {}
    }
}

/// What a step's badge says. Grey has not run yet, or is waiting on an
/// earlier step; amber ran but needs a look - edited since, or done with
/// warnings - and red ran and cannot finish.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum StepBadge {
    NotRun,
    Complete,
    Warning,
    Stale,
    Error,
}

impl StepBadge {
    pub(crate) fn of(state: crate::app::planning_pipeline::StageState, diagnostics: &[crate::app::planning_pipeline::StageDiagnostic]) -> Self {
        use crate::app::planning_pipeline::StageState;
        match state {
            StageState::Complete if diagnostics.iter().any(|diagnostic| !diagnostic.blocking) => Self::Warning,
            StageState::Complete => Self::Complete,
            StageState::Stale | StageState::Cancelled => Self::Stale,
            StageState::Failed => Self::Error,
            // Blocked is waiting on an earlier step rather than wrong itself:
            // that step carries the red.
            StageState::NotRun | StageState::Blocked | StageState::Queued | StageState::Running => Self::NotRun,
        }
    }

    pub(crate) fn icon(self) -> egui::ImageSource<'static> {
        match self {
            Self::NotRun => unthemed_icon!("step_not_run.svg"),
            Self::Complete => unthemed_icon!("step_complete.svg"),
            Self::Warning => unthemed_icon!("step_warning.svg"),
            Self::Stale => unthemed_icon!("step_pending.svg"),
            Self::Error => unthemed_icon!("step_error.svg"),
        }
    }
}

fn stage_tooltip(ui: &mut egui::Ui, status: &crate::ui::state::PlanningStageView) {
    stage_tooltip_parts(ui, status.state, status.blocked_by.map(SolidsStep::label), status.message.as_deref(), &status.diagnostics);
}

/// One step badge's hover: where it stands, what stopped it, and everything
/// its last run had to say.
///
/// Takes the parts rather than a view, so the Solids and Schedule step trees
/// say the same things in the same order about their own stages.
pub(crate) fn stage_tooltip_parts(
    ui: &mut egui::Ui,
    state: crate::app::planning_pipeline::StageState,
    blocked_by: Option<String>,
    message: Option<&str>,
    diagnostics: &[crate::app::planning_pipeline::StageDiagnostic],
) {
    ui.label(bold(&state.label()));
    if state == crate::app::planning_pipeline::StageState::Stale {
        ui.label(tr!("stage-stale-hint"));
    }
    // A blocked stage's message is usually this same line; it is said once.
    let blocker = blocked_by.map(|stage| tr!("stage-blocked-by", stage = stage));
    if let Some(blocker) = &blocker {
        ui.label(blocker);
    }
    if let Some(message) = message.filter(|message| blocker.as_deref() != Some(*message)) {
        ui.label(message);
    }
    if diagnostics.is_empty() {
        return;
    }
    ui.separator();
    ui.label(bold(&tr!("stage-diagnostics")));
    for entry in diagnostics.iter().take(12) {
        let text = match &entry.entity {
            Some(entity) => format!("{entity}: {}", entry.message),
            None => entry.message.clone(),
        };
        let color = if entry.blocking { ui.visuals().error_fg_color } else { ui.visuals().weak_text_color() };
        ui.label(egui::RichText::new(text).color(color));
    }
    if diagnostics.len() > 12 {
        ui.label(egui::RichText::new(format!("… {}", diagnostics.len() - 12)).color(ui.visuals().weak_text_color()));
    }
}

/// A step's Run Step, Run All and Cancel, for whichever pipeline it is in.
pub(crate) fn draw_stage_menu(response: &egui::Response, label: String, running: bool, [step, all, cancel]: [UiCommand; 3], commands: &mut Vec<UiCommand>) {
    context_menu_popup(response, label, |ui| {
        if ContextMenuAction::new(tr!("stage-run-step")).enabled(!running).show(ui).clicked() {
            commands.push(step);
            ui.close();
        }
        if ContextMenuAction::new(tr!("stage-run-all")).enabled(!running).show(ui).clicked() {
            commands.push(all);
            ui.close();
        }
        if ContextMenuAction::new(tr!("stage-cancel")).enabled(running).show(ui).clicked() {
            commands.push(cancel);
            ui.close();
        }
    });
}

/// Field List's helper column: every block model's columns, each with a +
/// that adds it to the Field List already mapped, or greyed once a field of
/// that name is there.
fn draw_column_tree(ui: &mut egui::Ui, rect: egui::Rect, document: &Document, block_models: &[OpenBlockModel], commands: &mut Vec<UiCommand>) {
    DataGrid::new("reserve_column_tree", rect, &tr!("planning-model-columns")).show(ui, |ui| {
        if block_models.is_empty() {
            grid_empty_state(ui, &tr!("planning-no-block-models-project"), None);
        }
        let tooltip = tr!("planning-add-column");
        let category = tr!("csv-block-model-category");
        for model in block_models {
            let numeric = model.model.numeric_variables();
            let categorical = model.model.categorical_variables();
            let columns: Vec<_> = numeric
                .iter()
                .map(|variable| (variable, false))
                .chain(categorical.iter().map(|variable| (variable, true)))
                .filter(|(variable, _)| !variable.special)
                .collect();
            let detail = columns.len().to_string();
            if !grid_group_row(ui, ("reserve_column_model", model.id), &model.name, &detail, 0) {
                continue;
            }
            for (variable, categorical) in columns {
                let added = document.reserve_fields().iter().any(|field| field.name == variable.name);
                let detail = if categorical { category.as_str() } else { "" };
                if grid_add_row(ui, ("reserve_add_column", model.id, &variable.name), &variable.name, detail, added, &tooltip) {
                    commands.push(UiCommand::AddReserveFieldFromColumn {
                        column: variable.name.clone(),
                        categorical,
                    });
                }
            }
        }
    });
}

/// How a field combines, as the Field List and the Schedule's tonnage choice
/// read it.
pub(crate) fn aggregation_label(document: &Document, aggregation: &ReserveAggregation) -> String {
    match aggregation {
        ReserveAggregation::Sum => tr!("planning-stat-sum"),
        ReserveAggregation::WeightedAverage { weight_field } => tr!(
            "planning-average-by",
            field = document.reserve_field(*weight_field).map_or_else(String::new, |field| field.name.clone())
        ),
        ReserveAggregation::VolumeAverage => tr!("reserve-average-by-volume"),
        ReserveAggregation::Category => tr!("csv-block-model-category"),
    }
}

fn draw_field_list(ui: &mut egui::Ui, rect: egui::Rect, editor: &mut EditorState, document: &Document, commands: &mut Vec<UiCommand>) {
    const FRACTIONS: [f32; 2] = [0.5, 0.5];
    let columns = [(tr!("planning-name"), FRACTIONS[0]), (tr!("planning-combines-as"), FRACTIONS[1])];
    let add_tooltip = tr!("reserve-new-field");
    let mut add = false;
    let fields = document.reserve_fields();
    let title = tr!("planning-field-list");
    let mut grid = DataGrid::new("reserve_field_list", rect, &title).add_button(&add_tooltip, &mut add);
    if !fields.is_empty() {
        grid = grid.columns(&columns);
    }
    let empty_clicked = grid.show(ui, |ui| {
        if fields.is_empty() {
            return grid_empty_state(ui, &tr!("planning-no-fields"), Some(&tr!("reserve-new-field")));
        }
        for field in fields {
            let combines = aggregation_label(document, &field.aggregation);
            // A weight others average by stays a Sum, and a category stays
            // one: neither has a choice to offer, so it is read rather than
            // picked. Anything else can be summed or averaged, by block volume
            // or by any other Sum field.
            let weights_others = fields
                .iter()
                .any(|other| other.aggregation == ReserveAggregation::WeightedAverage { weight_field: field.id });
            let weights: Vec<_> = fields.iter().filter(|other| other.id != field.id && other.aggregation == ReserveAggregation::Sum).collect();
            let fixed = field.aggregation == ReserveAggregation::Category || weights_others;
            let (response, cells) = grid_columns_row(ui, &FRACTIONS, &[&field.name, if fixed { &combines } else { "" }], false);
            let response = if weights_others {
                response.on_hover_text(tr!("reserve-weights-others"))
            } else {
                response
            };
            if !fixed {
                let mut choice = field.aggregation;
                let options = [
                    (ReserveAggregation::Sum, tr!("planning-stat-sum")),
                    (ReserveAggregation::VolumeAverage, tr!("reserve-average-by-volume")),
                ]
                .into_iter()
                .chain(weights.iter().map(|weight| {
                    (
                        ReserveAggregation::WeightedAverage { weight_field: weight.id },
                        tr!("planning-average-by", field = weight.name.clone()),
                    )
                }));
                if grid_cell_combo(ui, ("reserve_field_combines", field.id), cells[1], &mut choice, options, &combines) {
                    commands.push(UiCommand::SetReserveFieldAggregation {
                        field: field.id,
                        aggregation: choice,
                    });
                }
            }
            context_menu_popup(&response, &field.name, |ui| {
                if ContextMenuAction::new(tr!("planning-rename-field")).show(ui).clicked() {
                    commands.push(UiCommand::BeginRenameItem(crate::ui::state::RenameTarget::ReserveField(field.id)));
                    ui.close();
                }
                if ContextMenuAction::new(tr!("planning-delete-field")).show(ui).clicked() {
                    commands.push(UiCommand::DeleteReserveField(field.id));
                    ui.close();
                }
            });
        }
        grid_add_action_row(ui, &tr!("reserve-new-field"))
    });
    if add || empty_clicked {
        editor.new_reserve_field_open = true;
    }
    crate::ui::dialogs::reserve_fields::draw_new_reserve_field_dialog(ui, editor, document, commands);
}

/// The Block Models step's left column: the project's block models, with
/// their block count. Selecting one drives the mapping panel beside it.
fn draw_block_model_list(ui: &mut egui::Ui, rect: egui::Rect, editor: &mut EditorState, project: &UiProjectView) {
    DataGrid::new("reserve_block_model_list", rect, &tr!("planning-block-models"))
        .column_header(&tr!("planning-name"))
        .show(ui, |ui| {
            if project.block_models.is_empty() {
                grid_empty_state(ui, &tr!("planning-no-block-models-project"), None);
            }
            for entry in &project.block_models {
                // Name only: the block count is a property of the model, and
                // it is listed as one beside the others rather than being
                // spliced into every row of the list.
                let label = if entry.dirty { format!("{} *", entry.name) } else { entry.name.clone() };
                let response = grid_row(ui, GridRow::new(&label).selected(editor.planning_selected_block_model == Some(entry.id))).on_hover_text(tr!(
                    "planning-extents",
                    lower = format!("{:.1}, {:.1}, {:.1}", entry.lower.x, entry.lower.y, entry.lower.z),
                    upper = format!("{:.1}, {:.1}, {:.1}", entry.upper.x, entry.upper.y, entry.upper.z)
                ));
                if response.clicked() {
                    editor.planning_selected_block_model = Some(entry.id);
                }
            }
        });
}

/// Current mapping choice for one field, as the mapping combo's own value
/// type: the field's un/mapped state plus, when mapped, its source.
#[derive(Clone, PartialEq)]
enum MappingChoice {
    Unmapped,
    Constant,
    Column(String),
    /// The column per cubic metre, times each block's volume.
    PerVolume(String),
}

impl MappingChoice {
    fn of(mapping: &[crate::model::block_model::ReserveFieldMapping], field: ReserveFieldId) -> Self {
        match mapping.iter().find(|entry| entry.field == field).map(|entry| &entry.source) {
            None => Self::Unmapped,
            Some(ReserveMappingSource::Constant(_)) => Self::Constant,
            Some(ReserveMappingSource::Column(name)) => Self::Column(name.clone()),
            Some(ReserveMappingSource::PerVolume(name)) => Self::PerVolume(name.clone()),
        }
    }

    fn label(&self) -> String {
        match self {
            Self::Unmapped => tr!("io-unmapped"),
            Self::Constant => tr!("planning-constant"),
            Self::Column(name) => name.clone(),
            Self::PerVolume(name) => tr!("planning-mapping-per-volume", column = name.clone()),
        }
    }
}

/// The Block Models step's workspace: one row per field of the project's
/// Field List, saying where the selected model takes it from and what that
/// adds up to across the model.
fn draw_block_model_mapping(ui: &mut egui::Ui, rect: egui::Rect, document: &Document, model: &OpenBlockModel, blocks: usize, commands: &mut Vec<UiCommand>) {
    const FRACTIONS: [f32; 5] = [0.22, 0.3, 0.18, 0.15, 0.15];
    // A scan that failed, or one whose inputs could not be loaded, leaves the
    // figures absent; this is how it is asked for again without editing the
    // mapping to force a new request key. Registered before the grid, so the
    // grid's own controls sit above it and take their clicks.
    let stats_menu = |response: &egui::Response, commands: &mut Vec<UiCommand>| {
        context_menu_popup(response, &model.name, |ui| {
            if ContextMenuAction::new(tr!("planning-recompute-stats")).show(ui).clicked() {
                commands.push(UiCommand::RecomputeReserveStats(model.id));
                ui.close();
            }
        });
    };
    let pane = ui.interact(rect, ui.id().with(("reserve_stats_retry", model.id)), egui::Sense::click());
    stats_menu(&pane, commands);

    let columns = [
        (tr!("planning-name"), FRACTIONS[0]),
        (tr!("planning-source"), FRACTIONS[1]),
        (tr!("planning-stat-sum-avg"), FRACTIONS[2]),
        (tr!("planning-stat-min"), FRACTIONS[3]),
        (tr!("planning-stat-max"), FRACTIONS[4]),
    ];
    let detail = tr!("planning-block-count", blocks = blocks.separate_with_commas());
    let toggle = tr!("planning-used-reserving");
    let mut included = model.included_in_reserves;
    let numeric_columns: Vec<_> = model.model.numeric_variables().into_iter().map(|variable| variable.name.clone()).collect();
    let categorical_columns: Vec<_> = model.model.categorical_variables().into_iter().map(|variable| variable.name.clone()).collect();
    DataGrid::new("reserve_block_model_mapping", rect, &model.name)
        .title_detail(&detail)
        .title_toggle(&toggle, &mut included)
        .columns(&columns)
        .show(ui, |ui| {
            if document.reserve_fields().is_empty() {
                grid_empty_state(ui, &tr!("planning-no-fields"), None);
            }
            for field in document.reserve_fields() {
                let is_category = matches!(field.aggregation, ReserveAggregation::Category);
                let stats = (!is_category).then(|| model.reserve_totals.get(&field.id)).flatten();
                let [total, min, max] = match stats {
                    Some(stats) => [stats.total, stats.min, stats.max].map(stat_text),
                    None => Default::default(),
                };
                let (response, cells) = grid_columns_row(ui, &FRACTIONS, &[&field.name, "", &total, &min, &max], false);
                stats_menu(&response, commands);

                // Anything the figures can't show: why they are absent, or
                // what they leave out. A wait is said in their place; a
                // problem is a mark on the field, explained on hover.
                let mut problems = Vec::new();
                if !is_category {
                    match stats {
                        Some(stats) => {
                            problems.extend(stats.issue.as_ref().map(crate::model::ReserveFieldIssue::describe));
                            if stats.missing_values > 0 || stats.unusable_weights > 0 {
                                problems.push(tr!(
                                    "stage-model-data-gaps",
                                    missing = stats.missing_values.to_string(),
                                    weights = stats.unusable_weights.to_string()
                                ));
                            }
                        }
                        None => {
                            if let Some(error) = &model.reserve_totals_error {
                                problems.push(tr!("planning-stat-scan-failed", error = error.clone()));
                            } else {
                                let waiting = if model.reserve_totals_awaiting_restore {
                                    tr!("planning-stat-loading")
                                } else if model.reserve_totals_key.is_some() {
                                    tr!("planning-stat-scanning")
                                } else {
                                    tr!("planning-stat-not-scanned")
                                };
                                grid_cell_fixed(ui, cells[2].union(cells[4]), &waiting);
                            }
                        }
                    }
                }
                if !problems.is_empty() {
                    grid_cell_warning(ui, ("reserve_mapping_warning", model.id, field.id), cells[0], &problems.join("\n"));
                }

                let current = MappingChoice::of(&model.reserve_mapping, field.id);
                let mut choice = current.clone();
                let mut options = vec![CellOption::Choice(MappingChoice::Unmapped, tr!("io-unmapped"))];
                if !is_category {
                    options.push(CellOption::Choice(MappingChoice::Constant, tr!("planning-constant")));
                }
                let model_columns = if is_category { &categorical_columns } else { &numeric_columns };
                options.push(CellOption::Heading(tr!("planning-columns-heading")));
                options.extend(model_columns.iter().map(|name| CellOption::Choice(MappingChoice::Column(name.clone()), name.clone())));
                // A summed quantity can also come from a per-volume column -
                // tonnes from density - grouped after the plain columns, the
                // densities first, and each named for what it computes.
                if field.aggregation == ReserveAggregation::Sum && !model_columns.is_empty() {
                    options.push(CellOption::Heading(tr!("planning-per-m3-heading")));
                    let mut per_volume: Vec<_> = model_columns.iter().collect();
                    per_volume.sort_by_key(|name| !crate::model::block_model::looks_like_density(name));
                    options.extend(
                        per_volume
                            .into_iter()
                            .map(|name| CellOption::Choice(MappingChoice::PerVolume(name.clone()), tr!("planning-mapping-per-volume", column = name.clone()))),
                    );
                }
                // A constant's value shares the cell with the choice.
                let source = cells[1];
                let (combo_cell, value_cell) = if matches!(current, MappingChoice::Constant) {
                    let split = source.left() + source.width() * 0.5;
                    (
                        egui::Rect::from_min_max(source.min, egui::pos2(split, source.bottom())),
                        Some(egui::Rect::from_min_max(egui::pos2(split, source.top()), source.max)),
                    )
                } else {
                    (source, None)
                };
                if grid_cell_combo(ui, ("reserve_mapping_kind", model.id, field.id), combo_cell, &mut choice, options, &current.label()) && choice != current {
                    let source = match &choice {
                        MappingChoice::Unmapped => None,
                        MappingChoice::Constant => Some(ReserveMappingSource::Constant(0.0)),
                        MappingChoice::Column(name) => Some(ReserveMappingSource::Column(name.clone())),
                        MappingChoice::PerVolume(name) => Some(ReserveMappingSource::PerVolume(name.clone())),
                    };
                    commands.push(UiCommand::SetReserveMapping {
                        block_model: model.id,
                        field: field.id,
                        source,
                    });
                }
                if let Some(value_cell) = value_cell {
                    let mut value = model
                        .reserve_mapping
                        .iter()
                        .find(|entry| entry.field == field.id)
                        .and_then(|entry| match &entry.source {
                            ReserveMappingSource::Constant(value) => Some(*value),
                            ReserveMappingSource::Column(_) | ReserveMappingSource::PerVolume(_) => None,
                        })
                        .unwrap_or(0.0);
                    if grid_cell_number(ui, ("reserve_mapping_constant", model.id, field.id), value_cell, &mut value, "") {
                        commands.push(UiCommand::SetReserveMapping {
                            block_model: model.id,
                            field: field.id,
                            source: Some(ReserveMappingSource::Constant(value)),
                        });
                    }
                }
            }
        });
    if included != model.included_in_reserves {
        commands.push(UiCommand::SetReserveModelIncluded { block_model: model.id, included });
    }
}

/// One of a field's figures: whole numbers once they reach the thousands,
/// where the decimals are noise, and two places below that.
fn stat_text(value: Option<f64>) -> String {
    match value {
        None => "—".to_owned(),
        Some(value) if value.abs() >= 1000.0 => format!("{value:.0}").separate_with_commas(),
        Some(value) => format!("{value:.2}"),
    }
}

/// Which list a Solids Setup step reads beside its workspace, and so which
/// pane the explorer column stacks under the step list.
///
/// The lists live down there rather than taking a column of their own: they
/// are navigation, the same as the steps above them, and a column each left
/// the workspace they drive with a third of the window.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum StepList {
    Solids,
    BlockModels,
}

pub(crate) fn step_list(editor: &EditorState) -> Option<StepList> {
    if !editor.is_planning_setup() || editor.planning_page != PlanningPage::Solids || editor.solids_subpage != crate::ui::state::PlanningSubpage::Setup {
        return None;
    }
    match editor.planning_solids_step {
        SolidsStep::Solids | SolidsStep::Benching => Some(StepList::Solids),
        SolidsStep::BlockModels => Some(StepList::BlockModels),
        SolidsStep::FieldList | SolidsStep::Blasting | SolidsStep::DigStrips => None,
    }
}

pub(crate) fn draw_step_list(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    list: StepList,
    editor: &mut EditorState,
    project: &UiProjectView,
    document: &Document,
    commands: &mut Vec<UiCommand>,
) {
    match list {
        StepList::Solids => draw_solid_list(ui, rect, editor, document, commands),
        StepList::BlockModels => draw_block_model_list(ui, rect, editor, project),
    }
}

/// The Solids and Benching steps' list: the project's solids, each with the
/// kind of volume it is. Selecting one drives the panes beside it.
fn draw_solid_list(ui: &mut egui::Ui, rect: egui::Rect, editor: &mut EditorState, document: &Document, commands: &mut Vec<UiCommand>) {
    const FRACTIONS: [f32; 2] = [0.6, 0.4];
    let columns = [(tr!("planning-name"), FRACTIONS[0]), (tr!("destination-type"), FRACTIONS[1])];
    let add_tooltip = tr!("solids-new-solid");
    let mut add = false;
    let added = DataGrid::new("planning_solid_list", rect, &tr!("planning-solids"))
        .columns(&columns)
        .add_button(&add_tooltip, &mut add)
        .show(ui, |ui| {
            let mut add = false;
            if document.solids().is_empty() {
                add |= grid_empty_state(ui, &tr!("planning-no-solids"), Some(&add_tooltip));
            }
            for solid in document.solids() {
                let kind = kind_label(solid.kind);
                let (response, _) = grid_columns_row(ui, &FRACTIONS, &[&solid.name, &kind], editor.planning_selected_solid == Some(solid.id));
                if response.clicked() && editor.planning_selected_solid != Some(solid.id) {
                    editor.planning_selected_solid = Some(solid.id);
                    editor.planning_selected_bench = None;
                    ui.ctx().request_repaint();
                }
                context_menu_popup(&response, &solid.name, |ui| {
                    if ContextMenuAction::new(tr!("planning-rename-solid")).show(ui).clicked() {
                        commands.push(UiCommand::BeginRenameItem(crate::ui::state::RenameTarget::Solid(solid.id)));
                        ui.close();
                    }
                    if ContextMenuAction::new(tr!("planning-topography-update-action")).show(ui).clicked() {
                        crate::ui::dialogs::solids::open_topography_update(editor, document.solids(), Some(solid.id));
                        ui.close();
                    }
                    if ContextMenuAction::new(tr!("planning-delete-solid")).show(ui).clicked() {
                        commands.push(UiCommand::DeleteSolid(solid.id));
                        ui.close();
                    }
                });
            }
            if !document.solids().is_empty() {
                add |= grid_add_action_row(ui, &add_tooltip);
            }
            let body = ui.available_rect_before_wrap();
            if body.is_positive() {
                let response = ui.interact(body, ui.id().with("new_solid_space"), egui::Sense::click());
                context_menu_popup(&response, tr!("planning-solids"), |ui| {
                    if ContextMenuAction::new(tr!("solids-new-solid")).show(ui).clicked() {
                        editor.new_solid_open = true;
                        ui.close();
                    }
                    if ContextMenuAction::new(tr!("planning-topography-update-action"))
                        .enabled(!document.solids().is_empty())
                        .show(ui)
                        .clicked()
                    {
                        crate::ui::dialogs::solids::open_topography_update(editor, document.solids(), None);
                        ui.close();
                    }
                });
            }
            add
        });
    if add || added {
        editor.new_solid_open = true;
    }
}

/// The Solids step's right column: the selected solid's surfaces, kind and
/// block model. A pit is reserved against its model, so leaving that unset
/// is flagged on the row; a dump or stockpile is placed material and needs
/// none.
fn draw_solid_properties(ui: &mut egui::Ui, rect: egui::Rect, project: &UiProjectView, solid: &crate::model::Solid, commands: &mut Vec<UiCommand>) {
    const FRACTIONS: [f32; 2] = [0.4, 0.6];
    let columns = [(tr!("planning-property"), FRACTIONS[0]), (tr!("planning-value"), FRACTIONS[1])];
    let mut edits = Vec::new();
    DataGrid::new("planning_solid_properties", rect, &solid.name).columns(&columns).show(ui, |ui| {
        grid_columns_row(ui, &FRACTIONS, &[&tr!("planning-name"), &solid.name], false);

        let (_, cells) = grid_columns_row(ui, &FRACTIONS, &[&tr!("common-colour"), ""], false);
        let mut color = solid.color;
        if grid_cell_color(ui, ("solid_color", solid.id), cells[1], &mut color) {
            edits.push(SolidEdit::Color(color));
        }

        let (_, cells) = grid_columns_row(ui, &FRACTIONS, &[&tr!("destination-type"), ""], false);
        let mut kind = solid.kind;
        if grid_cell_combo(
            ui,
            ("solid_kind", solid.id),
            cells[1],
            &mut kind,
            SolidKind::ALL.into_iter().map(|kind| (kind, kind_label(kind))),
            &kind_label(solid.kind),
        ) && kind != solid.kind
        {
            edits.push(SolidEdit::Kind(kind));
        }

        let surfaces = triangulation_options(project);
        let (_, cells) = grid_columns_row(ui, &FRACTIONS, &[&tr!("tri-type-open-surface"), ""], false);
        let mut surface = solid.surface;
        if grid_cell_combo(
            ui,
            ("solid_surface", solid.id),
            cells[1],
            &mut surface,
            surfaces.clone(),
            &triangulation_label(project, solid.surface),
        ) && surface != solid.surface
        {
            edits.push(SolidEdit::Surface(surface));
        }

        let (_, cells) = grid_columns_row(ui, &FRACTIONS, &[&tr!("solids-topography"), ""], false);
        let mut topography = solid.topography;
        if grid_cell_combo(
            ui,
            ("solid_topography", solid.id),
            cells[1],
            &mut topography,
            surfaces,
            &triangulation_label(project, solid.topography),
        ) && topography != solid.topography
        {
            edits.push(SolidEdit::Topography(topography));
        }

        let (_, cells) = grid_columns_row(ui, &FRACTIONS, &[&tr!("ws-menubar-block-model"), ""], false);
        if solid.kind.requires_block_model() && solid.block_model.is_none() {
            grid_cell_warning(ui, ("solid_block_model_warning", solid.id), cells[0], &tr!("planning-set-block-model-reserve"));
        }
        let mut block_model = solid.block_model;
        if grid_cell_combo(
            ui,
            ("solid_block_model", solid.id),
            cells[1],
            &mut block_model,
            block_model_options(project),
            &block_model_label(project, solid.block_model),
        ) && block_model != solid.block_model
        {
            edits.push(SolidEdit::BlockModel(block_model));
        }
    });
    commands.extend(edits.into_iter().map(|edit| UiCommand::UpdateSolid { solid: solid.id, edit }));
}

/// The Solids step's render pane: the solid itself, drawn by the renderer into
/// an offscreen texture (see [`crate::rendering::graphics::solid_preview`])
/// and painted here over the rest of the loaded project.
///
/// A solid with only a design surface shows that surface; one that also names
/// a topography shows the closed volume between the two, with its enclosed
/// volume captioned. Right drag orbits, middle drag pans, the wheel zooms.
pub(crate) fn draw_solid_render(ui: &mut egui::Ui, rect: egui::Rect, editor: &mut EditorState, session: u32, commands: &mut Vec<UiCommand>) {
    let title = tr!("charging-preview");
    framed_render_pane(ui, rect, &title, |ui, body| {
        let caption_height = ui.text_style_height(&egui::TextStyle::Body) + 8.0;
        let image_rect = egui::Rect::from_min_max(body.min, egui::pos2(body.right(), (body.bottom() - caption_height).max(body.top())));
        if !image_rect.is_positive() {
            return;
        }

        // Paint first, then claim the area for input. Interaction is taken
        // with an id of its own rather than riding on an `Image` response, so
        // it stays the topmost interactive widget over the pane whichever
        // branch painted it.
        // A rebuild keeps the solid it is replacing on screen, so the image is
        // live for that too - not only once the new one lands.
        let ready = (editor.is_solids_view() && editor.solid_preview_texture.is_some())
            || matches!(
                editor.solid_preview_summary,
                crate::ui::state::SolidPreviewSummary::Ready { .. }
                    | crate::ui::state::SolidPreviewSummary::Building { showing_previous: true }
                    | crate::ui::state::SolidPreviewSummary::LoadingInputs { showing_previous: true }
            );
        match editor.solid_preview_texture {
            Some(texture_id) if ready => {
                let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                ui.painter().image(texture_id, image_rect, uv, egui::Color32::WHITE);
            }
            _ => {
                ui.painter().rect_filled(image_rect, 0.0, crate::ui::widgets::tree_row_colors(ui).1);
            }
        }
        let response = ui.interact(image_rect, ui.id().with("solid_preview_view"), egui::Sense::click_and_drag());

        let pixels_per_point = ui.ctx().pixels_per_point();
        let size_px = [
            (image_rect.width() * pixels_per_point).round().max(1.0) as u32,
            (image_rect.height() * pixels_per_point).round().max(1.0) as u32,
        ];
        // The renderer works a frame behind this measurement, so a resized
        // column needs one more frame to redraw at its new size.
        let mut view_changed = editor.solid_preview_size_px != size_px;
        editor.solid_preview_size_px = size_px;

        // Middle drag pans, right drag orbits about the ground under the
        // pointer and the wheel zooms, as in the main viewport. Left click
        // selects - in View, the one page here with anything to select - and
        // never moves the camera.
        let selects = editor.is_solids_view();
        let mut view = editor.solid_preview_view;
        view_changed |= crate::ui::widgets::preview_navigation::navigate(ui, &response, image_rect, &mut view, editor, true);
        editor.solid_preview_view = view;
        // A click that did not drag selects the solid under it. In View that is
        // how a dig block, which the tree does not list, is picked out for its
        // own figures. The click is addressed from the moment it is made - this
        // page, this project, this image - because it is resolved frames later,
        // and the UV is a fraction of the image: the renderer sizes its target
        // within limits of its own, and a pane outside them is drawn at one
        // size and would otherwise be picked against another.
        if response.clicked()
            && selects
            && let Some(pointer) = response.interact_pointer_pos()
        {
            editor.solid_preview_pick = Some(crate::ui::state::SolidPreviewPickRequest {
                session,
                owner: crate::ui::state::SolidPreviewPickOwner::SolidsView,
                generation: None,
                image: editor.solid_preview_image_revision,
                uv: crate::ui::widgets::preview_navigation::image_uv(image_rect, pointer),
            });
            ui.ctx().request_repaint();
        }
        if view_changed {
            ui.ctx().request_repaint();
        }
        // The viewport's own orientation gizmo, over the preview image and
        // driving the preview's orbit: one gizmo in the app, not two.
        if ready {
            let mut view = editor.solid_preview_view;
            crate::ui::widgets::preview_navigation::orientation_gizmo(ui, "solid_preview_orientation_gizmo", image_rect, &mut view, editor);
            editor.solid_preview_view = view;
        }

        // A built solid is only a preview until it is asked for by name; a
        // lone surface is already in the project, so there is nothing to add.
        let can_save = !editor.is_solids_view() && matches!(editor.solid_preview_summary, crate::ui::state::SolidPreviewSummary::Ready { volume: Some(_), .. });
        context_menu_popup(&response, &title, |ui| {
            if ContextMenuAction::new(tr!("planning-save-solid-project")).enabled(can_save).show(ui).clicked() {
                commands.push(UiCommand::SaveSolidPreviewToProject);
                ui.close();
            }
            if ContextMenuAction::new(tr!("gantt-reset-view")).show(ui).clicked() {
                commands.push(UiCommand::ResetSolidPreviewView);
                ui.close();
            }
        });

        let caption_rect = egui::Rect::from_min_max(egui::pos2(body.left() + 8.0, image_rect.bottom()), body.max);
        let caption = match &editor.solid_preview_summary {
            crate::ui::state::SolidPreviewSummary::NotRun => tr!("planning-not-run"),
            crate::ui::state::SolidPreviewSummary::Empty if !editor.is_solids_view() && editor.planning_selected_solid.is_none() => tr!("planning-no-solid-selected"),
            crate::ui::state::SolidPreviewSummary::Empty => tr!("planning-set-surface-inspect-solid"),
            crate::ui::state::SolidPreviewSummary::Unloaded => tr!("planning-load-solid-surfaces-inspect"),
            crate::ui::state::SolidPreviewSummary::LoadingInputs { .. } => tr!("planning-loading-solid-surfaces"),
            crate::ui::state::SolidPreviewSummary::Building { showing_previous: true } => tr!("planning-rebuilding-solid"),
            crate::ui::state::SolidPreviewSummary::Building { .. } => tr!("planning-building-solid"),
            crate::ui::state::SolidPreviewSummary::Ready { volume: Some(volume), .. } => {
                tr!("planning-solid-volume", volume = format!("{volume:.0}").separate_with_commas())
            }
            crate::ui::state::SolidPreviewSummary::Ready { volume: None, .. } if editor.is_solids_view() => tr!("planning-volume-unavailable"),
            crate::ui::state::SolidPreviewSummary::Ready {
                volume: None,
                waiting_on_unloaded: true,
                ..
            } => tr!("planning-surface-only-other-surface"),
            crate::ui::state::SolidPreviewSummary::Ready { volume: None, .. } => {
                tr!("planning-surface-only")
            }
            crate::ui::state::SolidPreviewSummary::Failed(message) => message.clone(),
        };
        let failed = matches!(editor.solid_preview_summary, crate::ui::state::SolidPreviewSummary::Failed(_));
        let color = if failed { ui.visuals().error_fg_color } else { ui.visuals().weak_text_color() };
        ui.put(
            caption_rect,
            egui::Label::new(egui::RichText::new(caption).color(color)).truncate().halign(egui::Align::Min),
        );
    });
}

/// The bordered, titled frame the render pane shares with the grids beside
/// it, handing its body rect to `content` instead of laying rows out.
fn framed_render_pane(ui: &mut egui::Ui, rect: egui::Rect, title: &str, content: impl FnOnce(&mut egui::Ui, egui::Rect)) {
    ui.scope_builder(egui::UiBuilder::new().id_salt(("planning_framed_pane", title.to_owned())).max_rect(rect), |ui| {
        ui.set_clip_rect(ui.clip_rect().intersect(rect));
        ui.painter().rect_filled(rect, 0.0, crate::ui::widgets::tree_row_colors(ui).1);
        let title_height = property_table_height(ui, 0);
        let title_rect = egui::Rect::from_min_size(rect.min + egui::vec2(8.0, 0.0), egui::vec2((rect.width() - 8.0).max(0.0), title_height));
        ui.put(title_rect, egui::Label::new(bold(title)).truncate().halign(egui::Align::Min));
        let body = egui::Rect::from_min_max(egui::pos2(rect.left(), rect.top() + title_height), rect.max);
        if body.is_positive() {
            content(ui, body);
        }
        ui.painter().rect_stroke(rect, 0.0, ui.visuals().widgets.noninteractive.bg_stroke, egui::StrokeKind::Inside);
    });
}

/// Regions and native panel resize handles painted after the workspace.
pub(crate) struct PlanningLayout {
    pub(crate) rect: egui::Rect,
    pub(crate) regions: Vec<egui::Rect>,
    pub(crate) grips: Vec<chrome::Grip>,
}

impl Default for PlanningLayout {
    fn default() -> Self {
        Self {
            rect: egui::Rect::NOTHING,
            regions: Vec::new(),
            grips: Vec::new(),
        }
    }
}

/// One column of a planning step.
fn island<R>(ui: &mut egui::Ui, layout: &mut PlanningLayout, id: &'static str, width: f32, content: impl FnOnce(&mut egui::Ui, egui::Rect) -> R) -> R {
    let response = Island::new(id, Side::Left).default_width(width).min_width(120.0).flush().show(ui, content);
    layout.regions.extend(response.regions);
    layout.grips.push(response.grip);
    response.inner
}

/// Least height a stacked pane may be dragged to before its neighbour stops
/// giving way. Two rows and a title strip: below that a grid says nothing.
const MIN_STACKED_PANE: f32 = 96.0;

/// The lower half of a column of two stacked panes.
///
/// Vertical stacking is how a column carries two grids without the workspace
/// paying for two columns of width, and the split between them is the user's:
/// the seam takes a grip of its own, the same three dots every other seam
/// here is marked with. The upper half is [`central_pane`], which takes
/// whatever this leaves.
fn stacked_lower<R>(ui: &mut egui::Ui, id: &'static str, content: impl FnOnce(&mut egui::Ui, egui::Rect) -> R) -> (egui::Rect, chrome::Grip) {
    stacked_lower_share(ui, id, 0.5, 0.0, content)
}

/// [`stacked_lower`], opening at `share` of the height on offer rather than
/// half, and never taking the `reserve` a pane claimed after it needs: for a
/// column of more than two panes, claimed from the bottom up.
pub(crate) fn stacked_lower_share<R>(
    ui: &mut egui::Ui,
    id: &'static str,
    share: f32,
    reserve: f32,
    content: impl FnOnce(&mut egui::Ui, egui::Rect) -> R,
) -> (egui::Rect, chrome::Grip) {
    // Bounded against the height actually on offer rather than a fixed pair of
    // limits, so a short window narrows both panes instead of letting one of
    // them push the other off the bottom.
    let available = ui.available_height() - reserve;
    let max = (available - MIN_STACKED_PANE).max(MIN_STACKED_PANE);
    let rect = egui::Panel::bottom(id)
        .resizable(true)
        .default_size(available * share)
        .min_size(MIN_STACKED_PANE.min(max))
        .max_size(max)
        .show_separator_line(chrome::show_separator_line(ui))
        .frame(chrome::region_frame(ui).inner_margin(egui::Margin::ZERO))
        .show(ui, |ui| {
            let rect = ui.available_rect_before_wrap();
            ui.set_clip_rect(ui.clip_rect().intersect(rect));
            content(ui, rect);
        })
        .response
        .rect;
    (rect, chrome::Grip::new(rect, chrome::Edge::Top, id))
}

/// Whatever is left once the islands have taken their columns, as one pane.
/// Returns what it claimed, for a caller that registers it itself.
fn central_pane(ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui, egui::Rect)) -> egui::Rect {
    egui::CentralPanel::default()
        .frame(chrome::region_frame(ui).inner_margin(egui::Margin::ZERO))
        .show(ui, |ui| content(ui, ui.available_rect_before_wrap()))
        .response
        .rect
}

/// A right-hand column of two stacked panes, each a region with the seam
/// between them marked. Used where a page reads two tables side by side down
/// one edge rather than across the workspace.
pub(crate) fn stacked_column(
    ui: &mut egui::Ui,
    layout: &mut PlanningLayout,
    id: &'static str,
    lower_id: &'static str,
    upper: impl FnOnce(&mut egui::Ui, egui::Rect),
    lower: impl FnOnce(&mut egui::Ui, egui::Rect),
) {
    let column = Island::new(id, Side::Right).default_width(360.0).min_width(200.0).bare().show(ui, |ui, _| {
        let (lower_rect, seam) = stacked_lower(ui, lower_id, lower);
        let upper_rect = central_pane(ui, upper);
        ([upper_rect, lower_rect], seam)
    });
    layout.regions.extend(column.inner.0);
    layout.grips.push(column.grip);
    layout.grips.push(column.inner.1);
}

/// [`central_pane`], for a caller in another module.
pub(crate) fn central_pane_of(ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui, egui::Rect)) -> egui::Rect {
    central_pane(ui, content)
}

fn central_island(ui: &mut egui::Ui, layout: &mut PlanningLayout, content: impl FnOnce(&mut egui::Ui, egui::Rect)) {
    let rect = central_pane(ui, content);
    layout.regions.push(rect);
}

/// The object tree with a second pane stacked under it.
///
/// The step's own figures - a solid's properties, a bench run's results - are
/// read against the objects they describe, so they share that column and the
/// seam between them is the user's, like every other seam here.
fn objects_column(
    ui: &mut egui::Ui,
    layout: &mut PlanningLayout,
    editor: &mut EditorState,
    project: &UiProjectView,
    commands: &mut Vec<UiCommand>,
    lower_id: &'static str,
    lower: impl FnOnce(&mut egui::Ui, egui::Rect, &mut EditorState, &mut Vec<UiCommand>),
) {
    let title = tr!("sequence-objects");
    let column = Island::new("planning_objects_island", Side::Right)
        .default_width(300.0)
        .min_width(160.0)
        .bare()
        .show(ui, |ui, _| {
            // The pane below is handed the editor and the command queue rather
            // than capturing them: the tree above needs both as well, and one
            // closure holding them would shut the other out.
            let (lower_rect, seam) = stacked_lower(ui, lower_id, |ui, rect| lower(ui, rect, editor, commands));
            let tree = central_pane(ui, |ui, rect| {
                framed_render_pane(ui, rect, &title, |ui, body| {
                    ui.scope_builder(egui::UiBuilder::new().id_salt("planning_solid_objects").max_rect(body), |ui| {
                        ui.set_clip_rect(ui.clip_rect().intersect(body));
                        ui.set_min_size(body.size());
                        ui.painter().rect_filled(body, 0.0, crate::ui::widgets::tree_row_colors(ui).0);
                        crate::ui::elements::explorer::draw_object_tree(ui, editor, project, commands);
                    });
                });
            });
            ([tree, lower_rect], seam)
        });
    layout.regions.extend(column.inner.0);
    layout.grips.push(column.grip);
    layout.grips.push(column.inner.1);
}

fn draw_solids_step(ui: &mut egui::Ui, layout: &mut PlanningLayout, editor: &mut EditorState, project: &UiProjectView, document: &Document, commands: &mut Vec<UiCommand>) {
    let session = project.active_session;
    // The solid list is in the explorer column under the step list; what is
    // read *about* the solid picked there belongs beside the objects it
    // describes, so it stacks under the object tree.
    // Open on a solid rather than an empty preview.
    if editor.planning_selected_solid.is_none_or(|id| document.solid(id).is_none()) {
        editor.planning_selected_solid = document.solids().first().map(|solid| solid.id);
    }
    let selected = editor.planning_selected_solid.and_then(|id| document.solid(id));
    objects_column(
        ui,
        layout,
        editor,
        project,
        commands,
        "planning_solids_properties_island",
        |ui, rect, _editor, commands| match selected {
            Some(solid) => draw_solid_properties(ui, rect, project, solid, commands),
            None => {
                let columns = [(tr!("planning-property"), 0.4), (tr!("planning-value"), 0.6)];
                DataGrid::new("planning_solid_properties_empty", rect, &tr!("planning-properties"))
                    .columns(&columns)
                    .show(ui, |_| {});
            }
        },
    );
    central_island(ui, layout, |ui, rect| draw_solid_render(ui, rect, editor, session, commands));
    crate::ui::dialogs::solids::draw_new_solid_dialog(ui, editor, project, commands);
    crate::ui::dialogs::solids::draw_topography_update_dialog(ui, editor, project, document.solids(), commands);
}

/// Shares of the benching grid's columns: the RLs a range runs between, then
/// the bench and flitch heights it is cut at.
const RANGE_FRACTIONS: [f32; 4] = [0.25, 0.25, 0.25, 0.25];

/// What is wrong with a range's flitch height, if anything.
fn flitch_warning(interval: &BenchInterval) -> Option<String> {
    if interval.flitch > 0.0 && (interval.bench / interval.flitch).ceil() > 64.0 {
        return Some(tr!("planning-too-many-flitches"));
    }
    (!interval.flitch_divides_bench()).then(|| {
        tr!(
            "planning-bench-not-whole-number",
            bench = format!("{:.2}", interval.bench),
            flitch = format!("{:.2}", interval.flitch)
        )
    })
}

/// A range named by the RLs it runs between, top first.
fn range_label(top: f64, base: f64) -> String {
    format!("{} – {}", format_rl(top), format_rl(base))
}

/// The Benching step's upper pane: one row per elevation range, top down,
/// with the bench and flitch heights it is cut at.
///
/// Ranges are contiguous, so only the plan's own top RL is typed; every
/// other range starts at the base of the one above it, shown greyed.
fn draw_benching_list(ui: &mut egui::Ui, rect: egui::Rect, plan: &mut BenchingPlan, changed: &mut bool) {
    let columns = [
        (tr!("planning-top-rl"), RANGE_FRACTIONS[0]),
        (tr!("planning-base-rl"), RANGE_FRACTIONS[1]),
        (tr!("planning-bench-column"), RANGE_FRACTIONS[2]),
        (tr!("planning-flitch-column"), RANGE_FRACTIONS[3]),
    ];
    let add_tooltip = tr!("planning-add-range");
    let mut add = false;
    let added = DataGrid::new("planning_bench_list", rect, &tr!("planning-benching"))
        .columns(&columns)
        .add_button(&add_tooltip, &mut add)
        .show(ui, |ui| {
            if plan.intervals.is_empty() {
                return grid_empty_state(ui, &tr!("planning-no-ranges"), Some(&add_tooltip));
            }
            let mut top = plan.top;
            let mut above = plan.top;
            let mut edit = None;
            for (index, interval) in plan.intervals.iter_mut().enumerate() {
                let (response, cells) = grid_columns_row(ui, &RANGE_FRACTIONS, &["", "", "", ""], false);
                if index == 0 {
                    *changed |= grid_cell_number(ui, ("bench_top", 0usize), cells[0], &mut top, "");
                } else {
                    grid_cell_fixed(ui, cells[0], &format_rl(above));
                }
                *changed |= grid_cell_number(ui, ("bench_base", index), cells[1], &mut interval.base, "");
                *changed |= grid_cell_number(ui, ("bench_height", index), cells[2], &mut interval.bench, " m");
                let warning = flitch_warning(interval);
                // The mark takes the end of the cell, so the number stops short of it.
                let flitch_cell = if warning.is_some() {
                    cells[3].with_max_x(cells[3].right() - CELL_WARNING_WIDTH)
                } else {
                    cells[3]
                };
                *changed |= grid_cell_number(ui, ("bench_flitch", index), flitch_cell, &mut interval.flitch, " m");
                if let Some(message) = &warning {
                    grid_cell_warning(ui, ("bench_flitch_warning", index), cells[3], message);
                }
                context_menu_popup(&response, range_label(above, interval.base), |ui| {
                    if ContextMenuAction::new(tr!("planning-insert-range-below")).show(ui).clicked() {
                        edit = Some((index, true));
                        ui.close();
                    }
                    if ContextMenuAction::new(tr!("planning-delete-range")).show(ui).clicked() {
                        edit = Some((index, false));
                        ui.close();
                    }
                });
                above = interval.base;
            }
            plan.top = top;
            match edit {
                // Splitting a range halves it: the new row takes the lower
                // part and inherits the heights, which is the edit a reader
                // then adjusts rather than one they have to reconstruct.
                Some((index, true)) => {
                    let above = if index == 0 { plan.top } else { plan.intervals[index - 1].base };
                    let interval = plan.intervals[index].clone();
                    let split = (above + interval.base) / 2.0;
                    plan.intervals.insert(index, BenchInterval { base: split, ..interval });
                    *changed = true;
                }
                Some((index, false)) => {
                    plan.intervals.remove(index);
                    *changed = true;
                }
                None => {}
            }
            grid_add_action_row(ui, &add_tooltip)
        });
    if add || added {
        let base = plan.intervals.last().map_or(plan.top, |interval| interval.base);
        plan.intervals.push(BenchInterval {
            base: base - DEFAULT_RANGE_DEPTH,
            bench: BenchingPlan::DEFAULT_BENCH,
            flitch: BenchingPlan::DEFAULT_FLITCH,
            styles: Vec::new(),
        });
        *changed = true;
    }
}

/// What one flitch position is called: the ends are named, the rest counted.
fn flitch_position_label(position: usize, count: usize) -> String {
    if position == 0 {
        return tr!("planning-top-flitch");
    }
    if position + 1 == count {
        return tr!("planning-bottom-flitch");
    }
    // A number, not a string: the catalog picks the ordinal ending by it.
    let index = position as u64 + 1;
    tr!("planning-flitch", index = index)
}

fn pattern_label(pattern: crate::model::FillStyle) -> String {
    match pattern {
        crate::model::FillStyle::Clear => tr!("grade-calendar-none"),
        crate::model::FillStyle::Crosses => tr!("common-crosses"),
        crate::model::FillStyle::Slashes => tr!("common-slashes"),
        crate::model::FillStyle::Solid => tr!("tri-type-solid-closed"),
    }
}

/// Shares of the flitch style grid's columns. The pattern cell holds both the
/// pattern and the colour it is drawn in.
const STYLE_FRACTIONS: [f32; 3] = [0.3, 0.2, 0.5];
/// Share of the pattern cell its dropdown takes; the colour has the rest.
const PATTERN_SPLIT: f32 = 0.62;

/// The Benching step's lower pane: how each flitch position is drawn, one row
/// per position, top down, under a rule naming its range when there are
/// several.
///
/// The position, not the flitch: every bench in a range is flitched the same
/// way, so its top flitches share one style.
fn draw_flitch_styles(ui: &mut egui::Ui, rect: egui::Rect, plan: &mut BenchingPlan, solid_color: [f32; 4], changed: &mut bool) {
    let columns = [
        (tr!("planning-flitch-column"), STYLE_FRACTIONS[0]),
        (tr!("planning-fill"), STYLE_FRACTIONS[1]),
        (tr!("planning-pattern"), STYLE_FRACTIONS[2]),
    ];
    DataGrid::new("planning_flitch_list", rect, &tr!("planning-flitch-styles"))
        .columns(&columns)
        .show(ui, |ui| {
            let ranges = plan.intervals.len();
            let mut above = plan.top;
            for (index, interval) in plan.intervals.iter_mut().enumerate() {
                let flitches = interval.flitch_count();
                if interval.styles.len() != flitches {
                    // Changing a height changes how many flitches a bench has.
                    // Positions that survive keep what they were given; new ones
                    // take the default shade for where they now sit.
                    interval.styles = (0..flitches)
                        .map(|position| {
                            interval
                                .styles
                                .get(position)
                                .copied()
                                .unwrap_or_else(|| crate::model::FlitchStyle::default_for(solid_color, position, flitches))
                        })
                        .collect();
                    *changed = true;
                }
                if ranges > 1 {
                    grid_separator_row(ui, &range_label(above, interval.base), 0);
                }
                for (position, style) in interval.styles.iter_mut().enumerate() {
                    let name = flitch_position_label(position, flitches);
                    let (_, cells) = grid_columns_row(ui, &STYLE_FRACTIONS, &[&name, "", ""], false);
                    *changed |= grid_cell_color(ui, ("flitch_fill", index, position), cells[1], &mut style.color);
                    let split = cells[2].left() + cells[2].width() * PATTERN_SPLIT;
                    let current = pattern_label(style.pattern);
                    *changed |= grid_cell_combo(
                        ui,
                        ("flitch_pattern", index, position),
                        cells[2].with_max_x(split),
                        &mut style.pattern,
                        crate::model::FillStyle::ALL.map(|pattern| (pattern, pattern_label(pattern))),
                        &current,
                    );
                    // A clear flitch draws no pattern, so has no colour to pick for one.
                    if style.pattern != crate::model::FillStyle::Clear {
                        *changed |= grid_cell_color(ui, ("flitch_pattern_color", index, position), cells[2].with_min_x(split), &mut style.pattern_color);
                    }
                }
                above = interval.base;
            }
        });
}

/// Shares of the results grid's columns.
const RESULT_FRACTIONS: [f32; 3] = [0.35, 0.35, 0.3];

/// The Benching step's pane under the object tree: every bench the plan
/// produces that holds some of the solid, bottom up, with its flitches beneath
/// it, each named by its base RL as the rest of the app names them.
///
/// Selecting a row picks that slice out in the preview.
fn draw_bench_results(ui: &mut egui::Ui, rect: egui::Rect, plan: &BenchingPlan, editor: &mut EditorState) {
    let columns = [
        (tr!("planning-bench-column"), RESULT_FRACTIONS[0]),
        (tr!("planning-flitch-column"), RESULT_FRACTIONS[1]),
        (tr!("planning-height"), RESULT_FRACTIONS[2]),
    ];
    DataGrid::new("planning_bench_results", rect, &tr!("planning-results")).columns(&columns).show(ui, |ui| {
        // A plan may run past the solid at either end - it is snapped out to
        // whole benches, and the topography cuts the solid short of the
        // design. Only the slices that actually hold some of it are worth
        // listing, or picking out in the preview.
        let occupied = editor.solid_preview_z_range;
        let holds_solid = |base: f64, top: f64| occupied.is_none_or(|(lowest, highest)| top > lowest + BAND_EPSILON && base < highest - BAND_EPSILON);
        let benches: Vec<_> = plan.benches().into_iter().filter(|bench| holds_solid(bench.base, bench.top())).collect();
        if benches.is_empty() {
            let note = if plan.intervals.is_empty() {
                tr!("planning-no-ranges")
            } else {
                tr!("planning-no-bench-holds-any")
            };
            grid_empty_state(ui, &note, None);
            return;
        }
        let mut row = |ui: &mut egui::Ui, cells: [&str; 3], selection: crate::ui::state::BenchSelection| {
            let selected = editor.planning_selected_bench == Some(selection);
            if grid_columns_row(ui, &RESULT_FRACTIONS, &cells, selected).0.clicked() {
                editor.planning_selected_bench = (!selected).then_some(selection);
            }
        };
        // Bottom up, so the list reads the way a pit is mined.
        for bench in benches.iter().rev() {
            let selection = crate::ui::state::BenchSelection {
                base: bench.base,
                top: bench.top(),
                is_flitch: false,
            };
            row(ui, [&format_rl(bench.base), "", &format!("{} m", format_rl(bench.height))], selection);
            for flitch in bench.flitches.iter().rev().filter(|flitch| holds_solid(flitch.base, flitch.top())) {
                let selection = crate::ui::state::BenchSelection {
                    base: flitch.base,
                    top: flitch.top(),
                    is_flitch: true,
                };
                row(ui, ["", &format_rl(flitch.base), &format!("{} m", format_rl(flitch.height))], selection);
            }
        }
    });
}

/// Slack on the solid's own extent when deciding whether a bench holds any of
/// it, so a bench boundary that lands exactly on the crest or the floor does
/// not list an empty slice.
const BAND_EPSILON: f64 = 1e-3;

/// Depth a range added by hand takes, so it lands visibly below the one above
/// rather than collapsed onto it. Its heights are the plan's own defaults.
const DEFAULT_RANGE_DEPTH: f64 = 120.0;

fn draw_benching_step(ui: &mut egui::Ui, layout: &mut PlanningLayout, editor: &mut EditorState, project: &UiProjectView, document: &Document, commands: &mut Vec<UiCommand>) {
    if editor.planning_selected_solid.is_none_or(|id| document.solid(id).is_none()) {
        editor.planning_selected_solid = document.solids().first().map(|solid| solid.id);
    }
    let selected = editor.planning_selected_solid.and_then(|id| document.solid(id));
    let solid_id = selected.map(|solid| solid.id);
    let solid_color = selected.map_or([1.0; 4], |solid| solid.color);
    let mut plan = selected.map(|solid| solid.benching.clone()).unwrap_or_default();
    let mut changed = false;
    // Results read against the objects they were measured from, so they take
    // the pane under the object tree; the solid list is in the explorer
    // column under the step list.
    let results = plan.clone();
    objects_column(ui, layout, editor, project, commands, "planning_bench_results_island", |ui, rect, editor, _commands| {
        if solid_id.is_some() {
            draw_bench_results(ui, rect, &results, editor);
        } else {
            DataGrid::new("planning_bench_results_empty", rect, &tr!("planning-results")).show(ui, |_| {});
        }
    });

    // This column arranges two panes rather than being one, so it carries no
    // frame of its own and they halve its height between them. It still closes
    // as a whole: the seam down its side drags both of them shut.
    let settings = Island::new("planning_bench_settings_column", Side::Left)
        .default_width(300.0)
        .min_width(140.0)
        .bare()
        .show(ui, |ui, _| {
            let (flitching, seam) = stacked_lower(ui, "planning_flitching_island", |ui, rect| {
                if solid_id.is_some() {
                    draw_flitch_styles(ui, rect, &mut plan, solid_color, &mut changed);
                } else {
                    DataGrid::new("planning_flitching_empty", rect, &tr!("planning-flitch-styles")).show(ui, |_| {});
                }
            });
            let benching = central_pane(ui, |ui, rect| {
                if solid_id.is_some() {
                    draw_benching_list(ui, rect, &mut plan, &mut changed);
                } else {
                    DataGrid::new("planning_benching_empty", rect, &tr!("planning-benching")).show(ui, |ui| {
                        grid_empty_state(ui, &tr!("planning-no-solid-selected"), None);
                    });
                }
            });
            ([flitching, benching], seam)
        });
    // The column is not a region itself: the two panes inside it are, and the
    // seam between them resizes the split the column's own seam cannot.
    layout.regions.extend(settings.inner.0);
    layout.grips.push(settings.grip);
    layout.grips.push(settings.inner.1);
    central_island(ui, layout, |ui, rect| draw_solid_render(ui, rect, editor, project.active_session, commands));
    crate::ui::dialogs::solids::draw_new_solid_dialog(ui, editor, project, commands);
    if let Some(solid_id) = solid_id
        && changed
    {
        plan.intervals.sort_by(|a, b| b.base.total_cmp(&a.base));
        commands.push(UiCommand::UpdateSolid {
            solid: solid_id,
            edit: SolidEdit::Benching(plan),
        });
    }
}

fn draw_solids_details(
    ui: &mut egui::Ui,
    layout: &mut PlanningLayout,
    editor: &mut EditorState,
    project: &UiProjectView,
    document: &Document,
    block_models: &[OpenBlockModel],
    commands: &mut Vec<UiCommand>,
) {
    match editor.planning_solids_step {
        SolidsStep::Blasting | SolidsStep::DigStrips => {}
        // The list is the workspace; the columns it is added from are a
        // helper beside it.
        SolidsStep::FieldList => {
            let columns = Island::new("reserve_column_island", Side::Right)
                .default_width(300.0)
                .min_width(160.0)
                .flush()
                .show(ui, |ui, rect| draw_column_tree(ui, rect, document, block_models, commands));
            layout.regions.extend(columns.regions);
            layout.grips.push(columns.grip);
            central_island(ui, layout, |ui, rect| draw_field_list(ui, rect, editor, document, commands));
        }
        SolidsStep::Solids => draw_solids_step(ui, layout, editor, project, document, commands),
        SolidsStep::Benching => draw_benching_step(ui, layout, editor, project, document, commands),
        // The model list is in the explorer column under the step list, so the
        // mapping has the workspace to itself.
        SolidsStep::BlockModels => {
            // Open on a model rather than an empty pane.
            if editor.planning_selected_block_model.is_none_or(|id| block_models.iter().all(|model| model.id != id)) {
                editor.planning_selected_block_model = project.block_models.first().map(|entry| entry.id);
            }
            central_island(ui, layout, |ui, rect| {
                if let Some(model) = editor.planning_selected_block_model.and_then(|id| block_models.iter().find(|model| model.id == id)) {
                    // The count is the project view's own, the same figure the
                    // list used to splice into the model's name.
                    let blocks = project.block_models.iter().find(|entry| entry.id == model.id).map_or(0, |entry| entry.block_count);
                    draw_block_model_mapping(ui, rect, document, model, blocks, commands);
                } else {
                    DataGrid::new("reserve_block_model_mapping_empty", rect, &tr!("planning-block-models")).show(ui, |ui| {
                        grid_empty_state(ui, &tr!("planning-no-block-models-project"), None);
                    });
                }
            });
        }
    }
}

/// Haulage Setup's panes: the road network's settings, or the truck classes
/// list beside the selected class.
fn draw_haulage_details(ui: &mut egui::Ui, layout: &mut PlanningLayout, editor: &mut EditorState, project: &UiProjectView, commands: &mut Vec<UiCommand>) {
    let session = project.active_session;
    match editor.haulage_setup_step {
        crate::ui::state::HaulageStep::Network => {
            central_island(ui, layout, |ui, rect| super::haulage::draw_network_settings(ui, rect, &project.haulage, session, commands));
        }
        crate::ui::state::HaulageStep::TruckClasses => {
            let plan = project.schedule.clone();
            // Open on a class rather than an empty table.
            if editor.schedule_selected_truck_class.is_none_or(|id| plan.trucks().class(id).is_none()) {
                editor.schedule_selected_truck_class = plan.trucks().classes.first().map(|c| c.id);
            }
            island(ui, layout, "schedule_truck_class_list_island", 320.0, |ui, rect| {
                super::schedule_trucking::draw_class_list(ui, rect, editor, &plan, session, commands)
            });
            // The class's figures above, its grade speeds below, with the
            // split between them the user's. It opens with the figures' rows
            // fitted and the bands, which run longer, taking the rest.
            let figures = crate::ui::widgets::data_grid::property_table_height(ui, 8) + 2.0;
            let share = (1.0 - figures / ui.available_height().max(1.0)).clamp(0.3, 0.85);
            let (bands, seam) = stacked_lower_share(ui, "schedule_truck_grade_speeds_pane", share, 0.0, |ui, rect| {
                super::schedule_trucking::draw_grade_speeds(ui, rect, editor, &plan, session, commands)
            });
            layout.regions.push(bands);
            layout.grips.push(seam);
            central_island(ui, layout, |ui, rect| {
                super::schedule_trucking::draw_class_properties(ui, rect, editor, &plan, session, commands)
            });
        }
    }
}

/// The Schedule Setup subpage's panes: a list beside the properties of the
/// row selected in it, the same shape the Solids steps use.
fn draw_schedule_details(ui: &mut egui::Ui, layout: &mut PlanningLayout, editor: &mut EditorState, project: &UiProjectView, document: &Document, commands: &mut Vec<UiCommand>) {
    use crate::ui::state::ScheduleStep;

    // Cloned rather than borrowed: the panes below take `editor` mutably to
    // hold their drafts and selection, and the plan they read is the active
    // project's own rather than the render-scene composite's.
    let plan = project.schedule.clone();
    // Every edit below is addressed to the project this plan was read from,
    // so one queued against a project that is closed before the frame's
    // commands are handled is refused rather than applied to its successor.
    let session = project.active_session;
    match editor.schedule_setup_step {
        ScheduleStep::Configuration => {
            central_island(ui, layout, |ui, rect| {
                super::schedule_setup::draw_configuration(ui, rect, editor, &plan, document, session, commands)
            });
        }
        ScheduleStep::Periods => {
            central_island(ui, layout, |ui, rect| super::schedule_periods::draw_periods(ui, rect, editor, &plan, session, commands));
        }
        ScheduleStep::LoaderClasses => {
            island(ui, layout, "schedule_class_list_island", 320.0, |ui, rect| {
                super::schedule_setup::draw_class_list(ui, rect, editor, &plan, session, commands)
            });
            central_island(ui, layout, |ui, rect| {
                super::schedule_setup::draw_class_properties(ui, rect, editor, &plan, session, commands)
            });
        }
        ScheduleStep::LoaderAgents => {
            island(ui, layout, "schedule_agent_list_island", 320.0, |ui, rect| {
                super::schedule_setup::draw_agent_list(ui, rect, editor, &plan, session, commands)
            });
            central_island(ui, layout, |ui, rect| {
                super::schedule_setup::draw_agent_properties(ui, rect, editor, &plan, session, commands)
            });
        }
        ScheduleStep::Delays => {
            island(ui, layout, "schedule_delay_index_island", 320.0, |ui, rect| {
                super::schedule_delays::draw_delay_index(ui, rect, editor, &plan, session, commands)
            });
            central_island(ui, layout, |ui, rect| super::schedule_delays::draw_delay_editor(ui, rect, editor, &plan, session, commands));
        }
        ScheduleStep::DrillBlast => {
            island(ui, layout, "schedule_blast_list_island", 320.0, |ui, rect| {
                super::schedule_drill_blast::draw_blast_list(ui, rect, editor, &plan, session, commands)
            });
            central_island(ui, layout, |ui, rect| {
                super::schedule_drill_blast::draw_settings(ui, rect, editor, &plan, session, commands)
            });
        }
        // The three destination pages share one shape: the list of that kind on
        // the left, the selected row's cells in the middle. They are separate
        // steps rather than one page with a filter because each is a thing that
        // can be marked Complete, Stale or Failed on its own.
        ScheduleStep::Stockpiles | ScheduleStep::Dumps | ScheduleStep::Crushers => {
            let kind = match editor.schedule_setup_step {
                ScheduleStep::Stockpiles => crate::model::schedule::DestinationKind::Stockpile,
                ScheduleStep::Dumps => crate::model::schedule::DestinationKind::Dump,
                _ => crate::model::schedule::DestinationKind::Crusher,
            };
            island(ui, layout, "schedule_destination_list_island", 320.0, |ui, rect| {
                super::schedule_destinations::draw_destination_list(ui, rect, editor, &plan, document, kind, session, commands)
            });
            // Stockpiles carry a third list: what the pile already holds, oldest
            // first. It sits beside the pile rather than on a page of its own,
            // because opening stock is a property of one stockpile.
            if kind == crate::model::schedule::DestinationKind::Stockpile {
                island(ui, layout, "schedule_opening_lots_island", 300.0, |ui, rect| {
                    super::schedule_destinations::draw_opening_lots(ui, rect, editor, &plan, document, session, commands)
                });
            }
            central_island(ui, layout, |ui, rect| {
                let table = super::schedule_destinations::draw_destination_properties(ui, rect, editor, &plan, document, kind, session, commands, &project.haulage);
                if kind == crate::model::schedule::DestinationKind::Stockpile {
                    let below = egui::Rect::from_min_max(egui::pos2(rect.left(), table.bottom() + ui.spacing().item_spacing.y), rect.max);
                    if below.is_positive() {
                        super::schedule_destinations::draw_lot_editor(ui, below, editor, &plan, document, session, commands);
                    }
                }
            });
            super::schedule_destinations::draw_new_destination_dialog(ui, editor, &plan, session, commands);
        }
        ScheduleStep::Destinations => {
            island(ui, layout, "schedule_rule_list_island", 380.0, |ui, rect| {
                super::schedule_destinations::draw_rule_list(ui, rect, editor, &plan, document, session, commands)
            });
            central_island(ui, layout, |ui, rect| {
                super::schedule_destinations::draw_rule_editor(ui, rect, editor, &plan, document, session, commands)
            });
        }
        ScheduleStep::Haulage => {
            island(ui, layout, "schedule_haulage_steps_island", 320.0, |ui, rect| draw_haulage_summary(ui, rect, editor));
            let (pits, seam) = stacked_lower_share(ui, "schedule_haul_pits_island", 0.3, 0.0, |ui, rect| draw_pit_connections(ui, rect, editor));
            layout.regions.push(pits);
            layout.grips.push(seam);
            central_island(ui, layout, |ui, rect| draw_destination_connections(ui, rect, editor));
        }
        ScheduleStep::TruckingRules => {
            island(ui, layout, "schedule_truck_rule_list_island", 380.0, |ui, rect| {
                super::schedule_trucking::draw_rule_list(ui, rect, editor, &plan, session, commands)
            });
            central_island(ui, layout, |ui, rect| {
                super::schedule_trucking::draw_rule_editor(ui, rect, editor, &plan, document, session, commands)
            });
        }
        ScheduleStep::Cashflow => {
            island(ui, layout, "schedule_cashflow_list_island", 380.0, |ui, rect| {
                super::schedule_cashflow::draw_rule_list(ui, rect, editor, &plan, document, session, commands)
            });
            central_island(ui, layout, |ui, rect| {
                super::schedule_cashflow::draw_rule_editor(ui, rect, editor, &plan, document, session, commands)
            });
        }
        ScheduleStep::Solids => {
            island(ui, layout, "schedule_solids_steps_island", 320.0, |ui, rect| draw_solids_summary(ui, rect, editor));
            central_island(ui, layout, |ui, rect| {
                super::schedule_setup::draw_solids_tonnage(ui, rect, editor, &plan, session, commands)
            });
        }
        ScheduleStep::Readiness => {
            central_island(ui, layout, |ui, rect| super::schedule_setup::draw_readiness(ui, rect, editor, &plan, document));
        }
    }
    crate::ui::dialogs::schedule::draw_new_loader_class_dialog(ui, editor, &plan, session, commands);
    crate::ui::dialogs::schedule::draw_new_loader_agent_dialog(ui, editor, &plan, session, commands);
    crate::ui::dialogs::schedule::draw_delete_agent_dialog(ui, editor, &plan, session, commands);
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_details(
    ui: &mut egui::Ui,
    editor: &mut EditorState,
    project: &UiProjectView,
    document: &Document,
    block_models: &[OpenBlockModel],
    commands: &mut Vec<UiCommand>,
    page: PlanningPage,
) -> PlanningLayout {
    let mut layout = PlanningLayout::default();
    let response = egui::CentralPanel::default().frame(egui::Frame::NONE).show(ui, |ui| match page {
        PlanningPage::Solids => draw_solids_details(ui, &mut layout, editor, project, document, block_models, commands),
        PlanningPage::Schedule => draw_schedule_details(ui, &mut layout, editor, project, document, commands),
        PlanningPage::Haulage => draw_haulage_details(ui, &mut layout, editor, project, commands),
    });
    layout.rect = response.response.rect;
    layout
}
