//! The schedule settings the optimiser is told that the rest of the plan does
//! not already say: the horizon and grades on Configuration's General table,
//! the rest under Advanced, and each stockpile's representation.
//!
//! Settings only. Runs start from the Gantt and Calendar run controls, and
//! their status is reported there; nothing here calculates. The settings are
//! authored and persisted in every build, including the browser, where
//! schedule calculation itself is unavailable.

use crate::{
    i18n::tr,
    model::{
        Document, ReserveAggregation,
        schedule::{
            DestinationId, SchedulePlan,
            experiment::{GradeUnit, MAX_SOLVE_SECONDS, MIN_INTERVAL_H, StockpileRepresentation},
        },
    },
    ui::{
        EditorState,
        state::{ScheduleEdit, ScheduleExperimentDraft, UiCommand},
        widgets::{
            context_menu::{ChecklistRow, Tick, checklist_popup},
            data_grid::{PropertyRows, PropertyTable, property_table_height},
            menu,
        },
    },
};

fn parse_positive(text: &str) -> Option<f64> {
    let value = text.trim().parse::<f64>().ok()?;
    (value.is_finite() && value > 0.0).then_some(value)
}

/// The typed text of every setting here, refreshed whenever the plan's own
/// values change underneath it.
fn draft<'a>(editor: &'a mut EditorState, plan: &SchedulePlan) -> &'a mut ScheduleExperimentDraft {
    let experiment = plan.experiment();
    let source = (
        experiment.planning_end_day,
        experiment.interval_h.to_bits(),
        experiment.solve_seconds.to_bits(),
        experiment.relative_gap.to_bits(),
        experiment.event_capacity,
    );
    if editor.schedule_experiment_draft.as_ref().is_none_or(|draft| draft.source != source) {
        editor.schedule_experiment_draft = Some(ScheduleExperimentDraft {
            source,
            end_day: experiment.planning_end_day.to_string(),
            interval_h: experiment.interval_h.to_string(),
            solve_seconds: experiment.solve_seconds.to_string(),
            relative_gap: experiment.relative_gap.to_string(),
        });
    }
    editor.schedule_experiment_draft.as_mut().expect("just ensured")
}

/// How many days the schedule runs, on the General table.
pub(crate) fn horizon_row(rows: &mut PropertyRows<'_>, editor: &mut EditorState, plan: &SchedulePlan, session: u32, edits: &mut Vec<UiCommand>) {
    let experiment = plan.experiment();
    let draft = draft(editor, plan);
    let end_day = draft.end_day.trim().parse::<u32>().ok().filter(|day| *day > 0);
    let invalid = crate::model::schedule::ScheduleError::InvalidExperimentSetting.message();
    let response = rows.field(&tr!("experiment-end-day"), &mut draft.end_day, end_day.is_none().then_some(invalid.as_str()));
    if response.lost_focus()
        && let Some(end_day) = end_day
        && end_day != experiment.planning_end_day
    {
        edits.push(UiCommand::schedule(
            session,
            ScheduleEdit::SetExperimentHorizon {
                end_day,
                interval_h: experiment.interval_h,
            },
        ));
    }
    response.on_hover_text(tr!("experiment-end-day-help"));
}

/// Which grades the schedule carries: a summary that opens a checklist of
/// every field that could be one.
///
/// Only fields that could be a blended grade are offered: a tonnes-weighted
/// numeric average. Anything else is not a unit question, it is a different
/// kind of column, and the grade gate says so in its own words.
pub(crate) fn grades_row(rows: &mut PropertyRows<'_>, plan: &SchedulePlan, document: &Document, session: u32, edits: &mut Vec<UiCommand>) {
    let experiment = plan.experiment();
    let candidates: Vec<_> = document
        .reserve_fields()
        .iter()
        .filter(|field| {
            matches!(field.aggregation, ReserveAggregation::WeightedAverage { weight_field } if Some(weight_field) == plan.tonnage_field())
                || experiment.grade_unit(field.id).is_some()
        })
        .collect();
    let carried: Vec<&str> = candidates
        .iter()
        .filter(|field| experiment.grade_unit(field.id).is_some())
        .map(|field| field.name.as_str())
        .collect();
    let summary = match carried.as_slice() {
        _ if candidates.is_empty() => tr!("experiment-no-grade-fields"),
        [] => tr!("experiment-grades-none"),
        names => names.join(", "),
    };
    let response = rows.select(&tr!("experiment-grades"), &summary).on_hover_text(tr!("experiment-grade-units-help"));
    checklist_popup(&response, tr!("experiment-grades"), 260.0, |ui| {
        if candidates.is_empty() {
            menu::menu_note(ui, tr!("experiment-no-grade-fields"));
        }
        for field in &candidates {
            let tracked = experiment.grade_unit(field.id).is_some();
            if ChecklistRow::new(&field.name, Tick::of(tracked, false)).show(ui).toggled {
                edits.push(UiCommand::schedule(
                    session,
                    ScheduleEdit::SetExperimentGradeUnit {
                        field: field.id,
                        unit: (!tracked).then_some(GradeUnit::Stored),
                    },
                ));
            }
        }
    });
}

/// The settings few projects need to change: how finely the calendar is
/// split and, where Improve is built in, how long it may search.
///
/// The event capacity is left to its derived value: it trades model size
/// against achievable production in a way nobody can set by eye.
pub(crate) fn draw_advanced(ui: &mut egui::Ui, rect: egui::Rect, editor: &mut EditorState, plan: &SchedulePlan, session: u32, commands: &mut Vec<UiCommand>) {
    let experiment = plan.experiment();
    let draft = draft(editor, plan);
    let interval_h = parse_positive(&draft.interval_h).filter(|hours| *hours >= MIN_INTERVAL_H);
    let solve_seconds = parse_positive(&draft.solve_seconds).filter(|seconds| *seconds <= MAX_SOLVE_SECONDS);
    let relative_gap = draft.relative_gap.trim().parse::<f64>().ok().filter(|gap| gap.is_finite() && (0.0..=1.0).contains(gap));
    let invalid = crate::model::schedule::ScheduleError::InvalidExperimentSetting.message();
    let improve = cfg!(feature = "scip");
    let rows_used = 2 + if improve { 2 } else { 0 };
    let table_rect = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), property_table_height(ui, rows_used).min(rect.height())));
    let mut edits = Vec::new();
    PropertyTable::new("schedule_experiment", table_rect, &tr!("experiment-advanced")).show(ui, |rows| {
        rows.header(&tr!("planning-property"), &tr!("planning-value"));
        let response = rows.field(&tr!("experiment-interval"), &mut draft.interval_h, interval_h.is_none().then_some(invalid.as_str()));
        if response.lost_focus()
            && let Some(interval_h) = interval_h
            && interval_h != experiment.interval_h
        {
            edits.push(UiCommand::schedule(
                session,
                ScheduleEdit::SetExperimentHorizon {
                    end_day: experiment.planning_end_day,
                    interval_h,
                },
            ));
        }
        response.on_hover_text(tr!("experiment-interval-help"));
        if !improve {
            return;
        }
        let response = rows.field(
            &tr!("experiment-solve-seconds"),
            &mut draft.solve_seconds,
            solve_seconds.is_none().then_some(invalid.as_str()),
        );
        let limit_committed = response.lost_focus();
        let response = rows.field(&tr!("experiment-relative-gap"), &mut draft.relative_gap, relative_gap.is_none().then_some(invalid.as_str()));
        if (limit_committed || response.lost_focus())
            && let (Some(seconds), Some(relative_gap)) = (solve_seconds, relative_gap)
            && (seconds != experiment.solve_seconds || relative_gap != experiment.relative_gap)
        {
            edits.push(UiCommand::schedule(session, ScheduleEdit::SetExperimentSolveLimits { seconds, relative_gap }));
        }
    });
    commands.append(&mut edits);
}

/// The optimisation rows on one stockpile's Setup page.
///
/// Appended to the pile's own property table rather than given a page of
/// their own: a representation is a property of one stockpile, exactly as its
/// reclaim order and its capacity are.
pub(crate) fn stockpile_rows(
    rows: &mut crate::ui::widgets::data_grid::PropertyRows<'_>,
    editor_chunks: &mut Option<(DestinationId, String, String)>,
    plan: &SchedulePlan,
    destination: DestinationId,
    session: u32,
    edits: &mut Vec<UiCommand>,
) {
    let experiment = plan.experiment();
    let mut representation = experiment.representation(destination);
    let selected = representation.label();
    let response = rows.combo(
        ("experiment_representation", format!("{destination:?}")),
        &tr!("experiment-representation"),
        &mut representation,
        &selected,
        [
            (StockpileRepresentation::NotConfigured, StockpileRepresentation::NotConfigured.label()),
            (StockpileRepresentation::Blended, StockpileRepresentation::Blended.label()),
            (StockpileRepresentation::Chunks, StockpileRepresentation::Chunks.label()),
        ],
    );
    response.on_hover_text(match experiment.representation(destination) {
        StockpileRepresentation::NotConfigured => tr!("experiment-representation-help"),
        StockpileRepresentation::Blended => tr!("experiment-blended-help"),
        StockpileRepresentation::Chunks => tr!("experiment-chunks-help"),
    });
    if representation != experiment.representation(destination) {
        edits.push(UiCommand::schedule(session, ScheduleEdit::SetStockpileRepresentation { destination, representation }));
    }
    if experiment.representation(destination) != StockpileRepresentation::Chunks {
        return;
    }
    let authored = experiment.stockpile(destination).map(|entry| entry.receiving_chunks.clone()).unwrap_or_default();
    let source = authored.iter().map(f64::to_string).collect::<Vec<_>>().join(", ");
    if editor_chunks.as_ref().is_none_or(|(id, held, _)| *id != destination || *held != source) {
        *editor_chunks = Some((destination, source.clone(), source.clone()));
    }
    let (_, _, text) = editor_chunks.as_mut().expect("just ensured");
    let parsed: Option<Vec<f64>> = text
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(|entry| entry.parse::<f64>().ok().filter(|value| value.is_finite() && *value > 0.0))
        .collect();
    let error = parsed.is_none().then(|| crate::model::schedule::ScheduleError::InvalidExperimentSetting.message());
    let response = rows.field(&tr!("experiment-receiving-chunks"), text, error.as_deref());
    let committed = response.lost_focus();
    response.on_hover_text(tr!("experiment-chunks-help"));
    if committed
        && let Some(capacities) = parsed
        && capacities != authored
    {
        edits.push(UiCommand::schedule(session, ScheduleEdit::SetStockpileChunks { destination, capacities }));
    }
}
