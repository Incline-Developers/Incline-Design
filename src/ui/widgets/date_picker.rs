//! A month calendar hung off a button, for picking one date.
//!
//! Opens on the month of the date it is showing (or today's), steps a month
//! at a time, and closes on the day clicked. Weeks start on Monday, the way
//! a mine roster is written.

use chrono::{Datelike, Duration, Local, Months, NaiveDate};

use crate::i18n::tr;

const CELL: egui::Vec2 = egui::vec2(30.0, 24.0);

/// Open a calendar under `response` while it is open, and return the date
/// picked, if one was this frame. `current` is the date the field shows.
pub(crate) fn date_popup(response: &egui::Response, current: Option<NaiveDate>) -> Option<NaiveDate> {
    let month_id = response.id.with("date_picker_month");
    let today = Local::now().date_naive();
    let mut picked = None;
    egui::Popup::menu(response)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .width(CELL.x * 7.0 + 12.0)
        .show(|ui| {
            let mut shown = ui
                .data(|data| data.get_temp::<NaiveDate>(month_id))
                .unwrap_or_else(|| current.unwrap_or(today).with_day(1).expect("day 1 exists"));
            ui.horizontal(|ui| {
                if ui.small_button("‹").on_hover_text(tr!("date-picker-previous")).clicked() {
                    shown = shown.checked_sub_months(Months::new(1)).unwrap_or(shown);
                }
                let title = egui::RichText::new(tr!("date-picker-month-year", month = month_name(shown.month()), year = shown.year().to_string())).strong();
                ui.add_sized(egui::vec2(CELL.x * 7.0 - 52.0, CELL.y), egui::Label::new(title));
                if ui.small_button("›").on_hover_text(tr!("date-picker-next")).clicked() {
                    shown = shown.checked_add_months(Months::new(1)).unwrap_or(shown);
                }
            });
            egui::Grid::new(month_id.with("grid")).spacing(egui::Vec2::ZERO).show(ui, |ui| {
                for weekday in [
                    tr!("date-picker-mon"),
                    tr!("date-picker-tue"),
                    tr!("date-picker-wed"),
                    tr!("date-picker-thu"),
                    tr!("date-picker-fri"),
                    tr!("date-picker-sat"),
                    tr!("date-picker-sun"),
                ] {
                    ui.add_sized(CELL, egui::Label::new(egui::RichText::new(weekday).weak()));
                }
                ui.end_row();
                // From the Monday on or before the 1st, six weeks: every
                // month fits, and the grid does not jump in height.
                let lead = i64::from(shown.weekday().num_days_from_monday());
                let mut day = shown - Duration::days(lead);
                for _ in 0..6 {
                    for _ in 0..7 {
                        let in_month = day.month() == shown.month();
                        let mut text = egui::RichText::new(day.day().to_string());
                        if !in_month {
                            text = text.weak();
                        }
                        if day == today {
                            text = text.underline();
                        }
                        let selected = current == Some(day);
                        if ui.add_sized(CELL, egui::Button::selectable(selected, text).frame_when_inactive(false)).clicked() {
                            picked = Some(day);
                        }
                        day += Duration::days(1);
                    }
                    ui.end_row();
                }
            });
            ui.data_mut(|data| data.insert_temp(month_id, shown));
            if picked.is_some() {
                ui.data_mut(|data| data.remove::<NaiveDate>(month_id));
                ui.close();
            }
        });
    picked
}

fn month_name(month: u32) -> String {
    match month {
        1 => tr!("date-picker-january"),
        2 => tr!("date-picker-february"),
        3 => tr!("date-picker-march"),
        4 => tr!("date-picker-april"),
        5 => tr!("date-picker-may"),
        6 => tr!("date-picker-june"),
        7 => tr!("date-picker-july"),
        8 => tr!("date-picker-august"),
        9 => tr!("date-picker-september"),
        10 => tr!("date-picker-october"),
        11 => tr!("date-picker-november"),
        _ => tr!("date-picker-december"),
    }
}
