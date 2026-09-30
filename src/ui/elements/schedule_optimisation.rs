//! The Optimisation settings: what the schedule optimiser is told that the
//! rest of the plan does not already say.
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
            experiment::{GradeUnit, StockpileRepresentation},
        },
    },
    ui::{
        EditorState,
        state::{ScheduleEdit, ScheduleExperimentDraft, UiCommand},
        widgets::data_grid::{PropertyTable, property_table_height},
    },
};

/// What the interval setting does, and what it deliberately does not do.
fn interval_help() -> String {
    tr!("experiment-interval-help")
}

fn parse_positive(text: &str) -> Option<f64> {
    let value = text.trim().parse::<f64>().ok()?;
    (value.is_finite() && value > 0.0).then_some(value)
}

/// The settings table.
///
/// Returns the rect it consumed so the caller can lay out beneath it.
pub(crate) fn draw_optimisation(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    editor: &mut EditorState,
    plan: &SchedulePlan,
    document: &Document,
    session: u32,
    commands: &mut Vec<UiCommand>,
) -> egui::Rect {
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
            event_capacity: experiment.event_capacity.map(|value| value.to_string()).unwrap_or_default(),
        });
    }
    let draft = editor.schedule_experiment_draft.as_mut().expect("just ensured");
    let end_day = draft.end_day.trim().parse::<u32>().ok().filter(|day| *day > 0);
    let interval_h = parse_positive(&draft.interval_h);
    let event_capacity = if draft.event_capacity.trim().is_empty() {
        Some(None)
    } else {
        draft
            .event_capacity
            .trim()
            .parse::<usize>()
            .ok()
            .filter(|value| (1..=crate::model::schedule::optimisation::SEGMENT_CEILING).contains(value))
            .map(Some)
    };
    let solve_seconds = parse_positive(&draft.solve_seconds);
    let relative_gap = draft.relative_gap.trim().parse::<f64>().ok().filter(|gap| gap.is_finite() && (0.0..=1.0).contains(gap));
    let invalid = crate::model::schedule::ScheduleError::InvalidExperimentSetting.message();

    // Only fields that could be a blended grade are offered: a tonnes-weighted
    // numeric average. Anything else is not a unit question, it is a different
    // kind of column, and the grade gate says so in its own words.
    let candidates: Vec<_> = document
        .reserve_fields()
        .iter()
        .filter(|field| matches!(field.aggregation, ReserveAggregation::WeightedAverage { .. }))
        .collect();
    let rows_used = 7 + candidates.len() + usize::from(candidates.is_empty());
    let table_rect = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), property_table_height(ui, rows_used).min(rect.height())));
    let mut edits = Vec::new();
    PropertyTable::new("schedule_experiment", table_rect, &tr!("experiment-section")).show(ui, |rows| {
        rows.header(&tr!("planning-property"), &tr!("planning-value"));
        let response = rows.field(&tr!("experiment-end-day"), &mut draft.end_day, end_day.is_none().then_some(invalid.as_str()));
        let horizon_committed = response.lost_focus();
        response.on_hover_text(tr!("experiment-end-day-help"));
        let response = rows.field(&tr!("experiment-interval"), &mut draft.interval_h, interval_h.is_none().then_some(invalid.as_str()));
        let interval_committed = response.lost_focus();
        response.on_hover_text(interval_help());
        if (horizon_committed || interval_committed)
            && let (Some(end_day), Some(interval_h)) = (end_day, interval_h)
            && (end_day != experiment.planning_end_day || interval_h != experiment.interval_h)
        {
            edits.push(UiCommand::schedule(session, ScheduleEdit::SetExperimentHorizon { end_day, interval_h }));
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
        let response = rows.field(
            &tr!("experiment-event-capacity"),
            &mut draft.event_capacity,
            event_capacity.is_none().then_some(invalid.as_str()),
        );
        let committed = response.lost_focus();
        response.on_hover_text(tr!("experiment-event-capacity-help"));
        if committed
            && let Some(capacity) = event_capacity
            && capacity != experiment.event_capacity
        {
            edits.push(UiCommand::schedule(session, ScheduleEdit::SetExperimentEventCapacity { capacity }));
        }
        // A unit per grade, stated. Nothing here infers one from the field's
        // name or from how big its values happen to be.
        if candidates.is_empty() {
            rows.readonly(&tr!("experiment-grades"), &tr!("experiment-no-grade-fields"), None, None);
        }
        for field in &candidates {
            let mut unit = experiment.grade_unit(field.id);
            let selected = unit.map_or_else(|| tr!("experiment-grade-unmapped"), GradeUnit::label);
            let response = rows.combo(
                ("experiment_grade", field.id.0),
                &field.name,
                &mut unit,
                &selected,
                [
                    (None, tr!("experiment-grade-unmapped")),
                    (Some(GradeUnit::Fraction), GradeUnit::Fraction.label()),
                    (Some(GradeUnit::Percent), GradeUnit::Percent.label()),
                ],
            );
            if response.changed() && unit != experiment.grade_unit(field.id) {
                edits.push(UiCommand::schedule(session, ScheduleEdit::SetExperimentGradeUnit { field: field.id, unit }));
            }
        }
    });
    commands.append(&mut edits);
    table_rect
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
