//! The Periods step of Schedule Setup, and how every schedule page writes a
//! time.
//!
//! The step names the dates the schedule runs between - Day 1 is the start
//! date, the end date sets the horizon - and lists each day, so days can be
//! marked with colours the Gantt and Animate show them in.
//!
//! The schedule's own clock counts hours from Day 1, 00:00. Every page writes
//! those hours through [`instant_label`] and [`day_label`], which add the date
//! once there is one: "Day 3 · 19/04/2026 06:00". The start date is set for
//! the frame by [`set_clock`], before any panel is drawn, rather than threaded
//! through every label on every page.

use std::cell::Cell;

use chrono::NaiveDate;

use crate::{
    i18n::tr,
    model::schedule::{
        SCHEDULE_PERIOD_H, SchedulePlan,
        periods::{date_text, date_time_at, date_time_text, last_date},
    },
    ui::{
        EditorState,
        state::{ScheduleEdit, UiCommand},
        widgets::{
            color::edit_srgba,
            context_menu::{ContextMenuAction, context_menu_popup},
            data_grid::{DataGrid, PropertyTable, grid_columns_row, grid_columns_row_height, property_table_height},
            date_picker::date_popup,
        },
    },
};

thread_local! {
    /// The date of Day 1 for the frame being drawn; see [`set_clock`].
    static START: Cell<Option<NaiveDate>> = const { Cell::new(None) };
}

/// Name Day 1 for every label drawn this frame. Called once, before the
/// panels, with the active project's start date.
pub(crate) fn set_clock(start: Option<NaiveDate>) {
    START.with(|cell| cell.set(start));
}

/// The date of Day 1 this frame, if the schedule has one.
pub(crate) fn clock_start() -> Option<NaiveDate> {
    START.with(Cell::get)
}

/// `hours` from Day 1, 00:00 as every page writes a time: "Day 3, 06:00",
/// or "Day 3 · 19/04/2026 06:00" once there is a start date.
pub(crate) fn instant_label(hours: f64) -> String {
    let minutes = (hours.max(0.0) * 60.0).round() as u64;
    let period_minutes = (SCHEDULE_PERIOD_H * 60.0) as u64;
    let day = minutes / period_minutes + 1;
    match clock_start().and_then(|start| date_time_at(start, hours)) {
        Some(at) => tr!("periods-day-date", day = day.to_string(), date = date_time_text(at)),
        None => {
            let within = minutes % period_minutes;
            tr!("gantt-day-time", day = day.to_string(), time = format!("{:02}:{:02}", within / 60, within % 60))
        }
    }
}

/// Period `period` (Day 1 is 0) as a heading: "Day 3", or "Day 3 ·
/// 19/04/2026" once there is a start date.
pub(crate) fn day_label(period: u32) -> String {
    let day = (u64::from(period) + 1).to_string();
    match clock_start().and_then(|start| start.checked_add_signed(chrono::Duration::days(i64::from(period)))) {
        Some(date) => tr!("periods-day-date", day = day, date = date_text(date)),
        None => tr!("gantt-day", day = day),
    }
}

/// A time as it is typed into a cell: the date and time once there is a
/// start date, "Day 3, 06:00" before.
pub(crate) fn editable_time(hours: f64) -> String {
    match clock_start().and_then(|start| date_time_at(start, hours)) {
        Some(at) => date_time_text(at),
        None => instant_label(hours),
    }
}

/// A period's colour, as the Gantt and Animate fill it.
pub(crate) fn period_color(color: [u8; 3]) -> egui::Color32 {
    egui::Color32::from_rgb(color[0], color[1], color[2])
}

/// The step: start and end dates and the horizon they make, then every day.
pub(crate) fn draw_periods(ui: &mut egui::Ui, rect: egui::Rect, editor: &mut EditorState, plan: &SchedulePlan, session: u32, commands: &mut Vec<UiCommand>) {
    let periods = plan.periods();
    let days = plan.experiment().planning_end_day;
    let start = periods.start_date;
    let mut edits = Vec::new();
    let table_rect = egui::Rect::from_min_size(
        rect.min,
        egui::vec2(rect.width(), property_table_height(ui, 4 + usize::from(start.is_none())).min(rect.height())),
    );
    PropertyTable::new("schedule_periods_dates", table_rect, &tr!("periods-dates")).show(ui, |rows| {
        rows.header(&tr!("planning-property"), &tr!("planning-value"));
        let start_text = start.map_or_else(|| tr!("periods-choose-date"), date_text);
        let response = rows.select(&tr!("periods-start-date"), &start_text);
        if let Some(date) = date_popup(&response, start)
            && Some(date) != start
        {
            edits.push(UiCommand::schedule(session, ScheduleEdit::SetStartDate(Some(date))));
        }
        let end = start.and_then(|start| last_date(start, days));
        let end_text = end.map_or_else(|| tr!("periods-needs-start"), date_text);
        let response = rows.select(&tr!("periods-end-date"), &end_text);
        if let Some(start) = start
            && let Some(date) = date_popup(&response, end)
        {
            // The last day the schedule covers, so the same day is one day.
            match u32::try_from((date - start).num_days() + 1) {
                Ok(end_day) if end_day > 0 => {
                    if end_day != days {
                        edits.push(UiCommand::schedule(
                            session,
                            ScheduleEdit::SetExperimentHorizon {
                                end_day,
                                interval_h: plan.experiment().interval_h,
                            },
                        ));
                    }
                }
                _ => crate::userspace_warn!("{}", tr!("periods-end-before-start")),
            }
        }
        super::schedule_optimisation::horizon_row(rows, editor, plan, session, &mut edits);
        if start.is_none() {
            rows.note(&tr!("periods-no-start-note"));
        }
    });

    let below = egui::Rect::from_min_max(egui::pos2(rect.left(), table_rect.bottom() + ui.spacing().item_spacing.y), rect.max);
    if below.is_positive() {
        draw_period_list(ui, below, editor, plan, session, &mut edits);
    }
    commands.append(&mut edits);
}

/// Every day of the horizon: its number, its date, its colour. Selecting
/// several and picking a colour on any of them marks them all.
fn draw_period_list(ui: &mut egui::Ui, rect: egui::Rect, editor: &mut EditorState, plan: &SchedulePlan, session: u32, edits: &mut Vec<UiCommand>) {
    const FRACTIONS: [f32; 3] = [0.3, 0.4, 0.3];
    let periods = plan.periods();
    let days = plan.experiment().planning_end_day;
    let columns = [
        (tr!("periods-period"), FRACTIONS[0]),
        (tr!("periods-date"), FRACTIONS[1]),
        (tr!("periods-colour"), FRACTIONS[2]),
    ];
    let selected = &mut editor.schedule_selected_periods;
    selected.retain(|period| *period < days);
    let mut anchor = editor.schedule_period_anchor;
    let detail = if selected.is_empty() {
        String::new()
    } else {
        tr!("periods-selected", count = selected.len().to_string())
    };
    DataGrid::new("schedule_period_list", rect, &tr!("periods-list"))
        .columns(&columns)
        .title_detail(&detail)
        .show(ui, |ui| {
            // Only the rows in view are drawn: a ten-year horizon is 3,650
            // of them, and the space of the rest stands in for them.
            let row = grid_columns_row_height(ui);
            let top = ui.cursor().top();
            let clip = ui.clip_rect();
            let first = (((clip.top() - top) / row).floor().max(0.0) as u32).min(days);
            // Past the clip by a screenful's margin: the clip can lag the pane
            // by a frame as the page is laid out, and a short list then ends
            // early.
            let last = ((((clip.bottom() - top) / row).ceil().max(0.0)) as u32 + 16).min(days);
            ui.add_space(first as f32 * row);
            for period in first..last {
                let date = periods.date_of_period(period).map(date_text).unwrap_or_default();
                let day = tr!("gantt-day", day = (period + 1).to_string());
                let is_selected = selected.contains(&period);
                let (response, cells) = grid_columns_row(ui, &FRACTIONS, &[&day, &date, ""], is_selected);
                if response.clicked() {
                    let modifiers = ui.input(|input| input.modifiers);
                    match (modifiers.shift, anchor) {
                        (true, Some(from)) => {
                            selected.clear();
                            selected.extend(from.min(period)..=from.max(period));
                        }
                        _ if modifiers.command => {
                            if !selected.remove(&period) {
                                selected.insert(period);
                            }
                            anchor = Some(period);
                        }
                        _ => {
                            selected.clear();
                            selected.insert(period);
                            anchor = Some(period);
                        }
                    }
                }
                // A colour picked on a selected row marks the whole
                // selection; on any other row, that row alone.
                let targets = || if is_selected { selected.iter().copied().collect() } else { vec![period] };
                let swatch = egui::Rect::from_min_size(cells[2].left_center() - egui::vec2(-4.0, 9.0), egui::vec2(40.0, 18.0));
                let marked = periods.color(period);
                // Unmarked reads as an empty box, not the checkerboard of a clear colour.
                let mut color = marked.map_or(ui.visuals().extreme_bg_color, period_color);
                let picked = ui
                    .scope_builder(egui::UiBuilder::new().id_salt(("period_color", period)).max_rect(swatch), |ui| {
                        edit_srgba(ui, &mut color, egui::color_picker::Alpha::Opaque)
                    })
                    .inner;
                if picked.changed() {
                    edits.push(UiCommand::schedule(
                        session,
                        ScheduleEdit::SetPeriodColors {
                            periods: targets(),
                            color: Some([color.r(), color.g(), color.b()]),
                        },
                    ));
                }
                let title = if is_selected && selected.len() > 1 {
                    tr!("periods-selected", count = selected.len().to_string())
                } else {
                    day.clone()
                };
                context_menu_popup(&response, title, |ui| {
                    if ContextMenuAction::new(tr!("periods-clear-colour"))
                        .enabled(marked.is_some() || is_selected)
                        .show(ui)
                        .clicked()
                    {
                        edits.push(UiCommand::schedule(session, ScheduleEdit::SetPeriodColors { periods: targets(), color: None }));
                        ui.close();
                    }
                });
            }
            ui.add_space(days.saturating_sub(last) as f32 * row);
        });
    editor.schedule_period_anchor = anchor;
}
