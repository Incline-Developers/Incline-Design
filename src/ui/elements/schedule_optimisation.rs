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
            SchedulePlan,
            experiment::{GradeUnit, MAX_SOLVE_SECONDS, MIN_INTERVAL_H, PlanStop},
        },
    },
    ui::{
        EditorState,
        state::{ScheduleEdit, ScheduleExperimentDraft, UiCommand},
        widgets::{
            context_menu::{ChecklistRow, Tick, checklist_popup},
            data_grid::{PropertyRows, PropertyTable},
            hint::Hint,
            menu,
        },
    },
};

fn parse_positive(text: &str) -> Option<f64> {
    let value = text.trim().parse::<f64>().ok()?;
    (value.is_finite() && value > 0.0).then_some(value)
}

/// A fraction written as a percentage, to six places at most, so 0.0001
/// reads 0.01 rather than 0.010000000000000002.
fn percent_text(fraction: f64) -> String {
    let text = format!("{:.6}", fraction * 100.0);
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// The typed text of every setting here, refreshed whenever the plan's own
/// values change underneath it.
fn draft<'a>(editor: &'a mut EditorState, plan: &SchedulePlan) -> &'a mut ScheduleExperimentDraft {
    let experiment = plan.experiment();
    let scale = plan.period_scale();
    let source = (
        scale,
        experiment.planning_end_day,
        experiment.interval_h.to_bits(),
        experiment.solve_seconds.to_bits(),
        experiment.relative_gap.to_bits(),
        experiment.event_capacity,
    );
    if editor.schedule_experiment_draft.as_ref().is_none_or(|draft| draft.source != source) {
        editor.schedule_experiment_draft = Some(ScheduleExperimentDraft {
            source,
            end_day: scale.count(experiment.planning_end_day).to_string(),
            interval_h: experiment.interval_h.to_string(),
            solve_seconds: experiment.solve_seconds.to_string(),
            relative_gap: percent_text(experiment.relative_gap),
        });
    }
    editor.schedule_experiment_draft.as_mut().expect("just ensured")
}

/// How many periods the schedule runs, on the General table. A horizon
/// that stops part-way through a period counts it: typing that count back
/// rounds the horizon up to the period's end.
pub(crate) fn horizon_row(rows: &mut PropertyRows<'_>, editor: &mut EditorState, plan: &SchedulePlan, session: u32, edits: &mut Vec<UiCommand>) {
    use crate::model::schedule::periods::PeriodUnit;
    let experiment = plan.experiment();
    let scale = plan.period_scale();
    let draft = draft(editor, plan);
    let end = draft.end_day.trim().parse::<u32>().ok().filter(|count| *count > 0).map(|count| scale.first_day(count));
    let invalid = crate::model::schedule::ScheduleError::InvalidExperimentSetting.message();
    let label = match scale.unit {
        PeriodUnit::Day => tr!("experiment-end-day"),
        PeriodUnit::Week => tr!("experiment-end-week"),
        PeriodUnit::Month => tr!("experiment-end-month"),
    };
    let response = rows.field(&label, &mut draft.end_day, end.is_none().then_some(invalid.as_str()));
    if response.lost_focus()
        && let Some(end_day) = end
        && scale.count(end_day) != scale.count(experiment.planning_end_day)
    {
        edits.push(UiCommand::schedule(
            session,
            ScheduleEdit::SetExperimentHorizon {
                end_day,
                interval_h: experiment.interval_h,
            },
        ));
    }
    response.hint(tr!("experiment-end-day-help"));
}

/// The calculation step, in hours, as a row of `rows`.
pub(crate) fn step_row(rows: &mut PropertyRows<'_>, editor: &mut EditorState, plan: &SchedulePlan, session: u32, edits: &mut Vec<UiCommand>) {
    let experiment = plan.experiment();
    let draft = draft(editor, plan);
    let interval_h = parse_positive(&draft.interval_h).filter(|hours| *hours >= MIN_INTERVAL_H);
    let invalid = crate::model::schedule::ScheduleError::InvalidExperimentSetting.message();
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
    response.hint(tr!("experiment-interval-help"));
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
    // Grades are averaged by tonnes, so until that field is chosen there is
    // nothing to offer, however many averages the Field List holds.
    let no_grades = if plan.tonnage_field().is_none() {
        tr!("experiment-grades-choose-tonnage")
    } else {
        tr!("experiment-no-grade-fields")
    };
    let summary = match carried.as_slice() {
        _ if candidates.is_empty() => no_grades.clone(),
        [] => tr!("experiment-grades-none"),
        names => names.join(", "),
    };
    let response = rows.select(&tr!("experiment-grades"), &summary).hint(tr!("experiment-grade-units-help"));
    checklist_popup(&response, tr!("experiment-grades"), 260.0, |ui| {
        if candidates.is_empty() {
            menu::menu_note(ui, no_grades.clone());
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
/// split and how long Improve may search.
///
/// The event capacity is left to its derived value: it trades model size
/// against achievable production in a way nobody can set by eye.
pub(crate) fn draw_advanced(ui: &mut egui::Ui, rect: egui::Rect, editor: &mut EditorState, plan: &SchedulePlan, session: u32, commands: &mut Vec<UiCommand>) {
    let experiment = plan.experiment();
    let draft = draft(editor, plan);
    let interval_h = parse_positive(&draft.interval_h).filter(|hours| *hours >= MIN_INTERVAL_H);
    let solve_seconds = parse_positive(&draft.solve_seconds).filter(|seconds| *seconds <= MAX_SOLVE_SECONDS);
    let relative_gap = draft
        .relative_gap
        .trim()
        .trim_end_matches('%')
        .trim_end()
        .parse::<f64>()
        .ok()
        .filter(|percent| percent.is_finite() && (0.0..=100.0).contains(percent))
        .map(|percent| percent / 100.0);
    let invalid = crate::model::schedule::ScheduleError::InvalidExperimentSetting.message();
    let mut edits = Vec::new();
    PropertyTable::new("schedule_experiment", rect, &tr!("experiment-advanced")).show(ui, |rows| {
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
        response.hint(tr!("experiment-interval-help"));
        let response = rows.field(
            &tr!("experiment-solve-seconds"),
            &mut draft.solve_seconds,
            solve_seconds.is_none().then_some(invalid.as_str()),
        );
        let limit_committed = response.lost_focus();
        let response = rows.field(&tr!("experiment-relative-gap"), &mut draft.relative_gap, relative_gap.is_none().then_some(invalid.as_str()));
        if (limit_committed || response.lost_focus())
            && let (Some(seconds), Some(relative_gap)) = (solve_seconds, relative_gap)
            && (seconds != experiment.solve_seconds || percent_text(relative_gap) != percent_text(experiment.relative_gap))
        {
            edits.push(UiCommand::schedule(session, ScheduleEdit::SetExperimentSolveLimits { seconds, relative_gap }));
        }
        // What the plan ordering the automatic bars' ground weighs later
        // value by.
        let (response, typed) = rows.committed_entry(
            "schedule_discount_rate",
            &tr!("experiment-discount-rate"),
            &percent_text(experiment.discount_rate),
            Some("%"),
        );
        response.hint(tr!("experiment-discount-rate-help"));
        if let Some(typed) = typed {
            match typed.trim().trim_end_matches('%').trim_end().parse::<f64>() {
                Ok(percent) if percent.is_finite() && (0.0..=100.0).contains(&percent) => {
                    edits.push(UiCommand::schedule(session, ScheduleEdit::SetDiscountRate(percent / 100.0)));
                }
                _ => crate::userspace_warn!("{}", invalid),
            }
        }
        plan_stop_rows(rows, experiment.plan_stop, session, &mut edits);
    });
    commands.append(&mut edits);
}

/// When the long-term plan stops refining its order: at a round adding less
/// than a share of its value, after a number of rounds, or at a time limit,
/// picked across the row with the figure for the one picked beside it.
fn plan_stop_rows(rows: &mut PropertyRows<'_>, stop: PlanStop, session: u32, edits: &mut Vec<UiCommand>) {
    rows.section(&tr!("experiment-plan-stop"));
    let labels = [
        tr!("experiment-plan-stop-improvement"),
        tr!("experiment-plan-stop-iterations"),
        tr!("experiment-plan-stop-time"),
    ];
    let (selected, mut value, unit) = match stop {
        PlanStop::Improvement(share) => (0, share * 100.0, "%".to_owned()),
        PlanStop::Rounds(rounds) => (1, f64::from(rounds), tr!("experiment-plan-stop-iterations-unit")),
        PlanStop::Seconds(seconds) => (2, seconds, "s".to_owned()),
    };
    let (picked, committed, response) = rows.choice_entry("plan_stop", &labels, selected, &mut value, &unit);
    response.hint(tr!("experiment-plan-stop-help"));
    // A newly picked rule starts from its own default figure.
    let wanted = match picked {
        Some(0) => Some(PlanStop::default()),
        Some(1) => Some(PlanStop::Rounds(DEFAULT_PLAN_ROUNDS)),
        Some(_) => Some(PlanStop::Seconds(DEFAULT_PLAN_SECONDS)),
        None if committed => Some(match stop {
            PlanStop::Improvement(_) => PlanStop::Improvement(value / 100.0),
            PlanStop::Rounds(_) => PlanStop::Rounds(if value.fract() == 0.0 && value >= 1.0 && value <= f64::from(u32::MAX) {
                value as u32
            } else {
                0
            }),
            PlanStop::Seconds(_) => PlanStop::Seconds(value),
        }),
        None => None,
    };
    match wanted {
        Some(next) if next.is_valid() && next != stop => edits.push(UiCommand::schedule(session, ScheduleEdit::SetPlanStop(next))),
        Some(next) if !next.is_valid() => crate::userspace_warn!("{}", crate::model::schedule::ScheduleError::InvalidExperimentSetting.message()),
        _ => {}
    }
}

/// The figures a newly picked rule for the long-term plan starts from.
const DEFAULT_PLAN_ROUNDS: u32 = 10;
const DEFAULT_PLAN_SECONDS: f64 = 20.0;
