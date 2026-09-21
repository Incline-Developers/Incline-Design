//! The experimental Optimisation section: what the blended optimiser is told,
//! and what its last run said.
//!
//! Developer configuration, not final production UI. It reuses the property
//! table every other Schedule page uses and builds nothing of its own: there
//! is no stockpile designer here, and there is no second reporting tab.
//!
//! Nothing on this page reaches the dispatcher. Run Period, Run All Periods,
//! the Gantt, the calendar production rows and the animation are unchanged by
//! everything below; the answer is retained separately and labelled stale the
//! moment an edit can have changed it.

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
        fonts::bold,
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

/// The settings table, the run controls, and the last result.
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
    let draft = editor.schedule_experiment_draft.as_mut().expect("just ensured");
    let end_day = draft.end_day.trim().parse::<u32>().ok().filter(|day| *day > 0);
    let interval_h = parse_positive(&draft.interval_h);
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
    let rows_used = 6 + candidates.len() + usize::from(candidates.is_empty());
    let table_rect = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), property_table_height(ui, rows_used).min(rect.height())));
    let mut edits = Vec::new();
    PropertyTable::new("schedule_experiment", table_rect, &tr!("experiment-section")).show(ui, |rows| {
        rows.header(&tr!("planning-property"), &tr!("planning-value"));
        let response = rows.field(&tr!("experiment-end-day"), &mut draft.end_day, end_day.is_none().then_some(invalid.as_str()));
        let horizon_committed = response.lost_focus();
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

    let body = egui::Rect::from_min_max(
        egui::pos2(rect.left(), table_rect.bottom() + ui.spacing().item_spacing.y),
        egui::pos2(rect.right(), rect.bottom()),
    );
    if !body.is_positive() {
        return table_rect;
    }
    let view = editor.experimental_blend.clone();
    ui.scope_builder(egui::UiBuilder::new().max_rect(body), |ui| {
        ui.set_clip_rect(ui.clip_rect().intersect(body));
        egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.add_enabled(!view.running, egui::Button::new(tr!("experiment-run"))).clicked() {
                    commands.push(UiCommand::RunExperimentalOptimisation);
                }
                if ui.add_enabled(view.running, egui::Button::new(tr!("experiment-cancel"))).clicked() {
                    commands.push(UiCommand::CancelExperimentalOptimisation);
                }
                let phase = if view.running {
                    tr!("experiment-phase-running")
                } else if !view.have_result {
                    tr!("experiment-phase-idle")
                } else if view.current {
                    tr!("experiment-phase-current")
                } else {
                    tr!("experiment-phase-stale")
                };
                ui.label(phase);
            });
            ui.add_space(6.0);
            for (label, value) in &view.rows {
                ui.horizontal(|ui| {
                    ui.add(egui::Label::new(bold(label)).truncate());
                    ui.add(egui::Label::new(value).wrap());
                });
            }
            if !view.notes.is_empty() {
                ui.add_space(6.0);
                ui.add(egui::Label::new(bold(&tr!("experiment-limitations"))).wrap());
                for note in &view.notes {
                    ui.add(egui::Label::new(egui::RichText::new(note).color(ui.visuals().weak_text_color())).wrap());
                }
            }
            if !view.diagnostics.is_empty() {
                ui.add_space(6.0);
                ui.add(egui::Label::new(bold(&tr!("experiment-diagnostics"))).wrap());
                for diagnostic in &view.diagnostics {
                    ui.add(egui::Label::new(egui::RichText::new(diagnostic).color(ui.visuals().error_fg_color)).wrap());
                }
            }
        });
    });
    table_rect
}

/// The two experimental rows on one stockpile's Setup page.
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
