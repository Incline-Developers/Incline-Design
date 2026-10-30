//! Animate's Record Video dialog: what part of the schedule to record, how
//! fast it plays, and at what frame rate. Where it is saved, and as which
//! format, is asked by the save dialog the Record button opens.
//!
//! The schedule's periods (its days) are listed under the weeks, months and
//! years they fall in - only the levels that divide them, so a run inside one
//! month is never wrapped in a single month row. Any days can be ticked, and
//! the recording plays the ticked ones back to back. Shift-click ticks or
//! clears every day between the last one clicked and this one.

use std::collections::HashSet;

use chrono::{Datelike, NaiveDate};

use crate::{
    i18n::tr,
    model::schedule::{SCHEDULE_PERIOD_H, delays::parse_time},
    ui::{
        EditorState,
        elements::schedule_periods::{clock_start, day_label, editable_time},
        state::{PlaybackSpan, UiCommand, VideoExportRequest},
        widgets::{
            context_menu::{ChecklistRow, Tick},
            date_picker::{date_popup, month_name},
            menu::{self, DragableMenu, MenuButton, MenuField},
            toolbar::GROUP_CORNER_RADIUS,
        },
    },
};

const DEFAULT_FPS: f64 = 30.0;
/// The sizes offered, as whole factors the viewport is shrunk by.
/// The sizes offered by name, (width, height, label); the viewport's own is
/// offered beside them.
const PRESETS: [(u32, u32, &str); 4] = [(1280, 720, "720p"), (1920, 1080, "1080p"), (2560, 1440, "1440p"), (3840, 2160, "4K")];
/// The size a new recording opens at: 1080p.
const DEFAULT_SIZE: (u32, u32) = (1920, 1080);
/// The largest side the size fields take; AV1's own limit is far above it.
const MAX_SIDE: u32 = 7680;
const MAX_FPS: f64 = 240.0;
/// The list's height before it scrolls.
const LIST_HEIGHT: f32 = 260.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RangeMode {
    Periods,
    Custom,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SpeedMode {
    Playback,
    Duration,
}

#[derive(Clone, Debug)]
pub(crate) struct VideoExportDialog {
    range: RangeMode,
    /// Whether each period (day) is recorded; all are to begin with.
    selected: Vec<bool>,
    /// Group rows opened, by key.
    expanded: HashSet<String>,
    /// The last day clicked, which a shift-click ticks from.
    anchor: Option<usize>,
    start: String,
    end: String,
    speed: SpeedMode,
    span: PlaybackSpan,
    seconds: f64,
    /// The video's length as typed, `mm:ss`.
    duration: String,
    fps: f64,
    /// The video's size in pixels, rounded to even when recorded.
    width: u32,
    height: u32,
    /// What is drawn over the scene in the video.
    destination_labels: bool,
    machine_labels: bool,
}

impl VideoExportDialog {
    /// Opened at the timeline's own playback speed, over the whole run.
    pub(crate) fn new(editor: &EditorState) -> Self {
        let horizon = editor.schedule_animation_horizon_h.max(0.0);
        let mut dialog = Self {
            range: RangeMode::Periods,
            selected: vec![true; period_count(horizon)],
            expanded: HashSet::new(),
            anchor: None,
            start: editable_time(0.0),
            end: editable_time(horizon),
            speed: SpeedMode::Playback,
            span: editor.schedule_animation_play_span,
            seconds: editor.schedule_animation_play_seconds,
            duration: String::new(),
            fps: DEFAULT_FPS,
            width: DEFAULT_SIZE.0,
            height: DEFAULT_SIZE.1,
            destination_labels: true,
            machine_labels: true,
        };
        let segments = dialog.segments(horizon).unwrap_or_default();
        dialog.duration = duration_text(dialog.playback_seconds(&segments));
        dialog
    }

    /// The stretches of the schedule recorded, in order, or `None` while the
    /// custom range does not read as one.
    fn segments(&self, horizon: f64) -> Option<Vec<(f64, f64)>> {
        match self.range {
            RangeMode::Periods => {
                let mut segments: Vec<(f64, f64)> = Vec::new();
                for (period, _) in self.selected.iter().enumerate().filter(|(_, on)| **on) {
                    let start = period as f64 * SCHEDULE_PERIOD_H;
                    let end = ((period + 1) as f64 * SCHEDULE_PERIOD_H).min(horizon);
                    match segments.last_mut() {
                        Some(last) if last.1 >= start => last.1 = end,
                        _ => segments.push((start, end)),
                    }
                }
                Some(segments)
            }
            RangeMode::Custom => {
                let start = parse_time(&self.start, clock_start())?;
                let end = parse_time(&self.end, clock_start())?;
                (start < end && end <= horizon + 1e-9).then(|| vec![(start, end)])
            }
        }
    }

    /// Seconds the recording lasts at the set playback speed.
    fn playback_seconds(&self, segments: &[(f64, f64)]) -> f64 {
        hours(segments) * self.seconds / self.span.hours()
    }

    /// Hours of schedule per second of video, or `None` while the speed
    /// does not read as one.
    fn hours_per_second(&self, segments: &[(f64, f64)]) -> Option<f64> {
        let rate = match self.speed {
            SpeedMode::Playback => self.span.hours() / self.seconds,
            SpeedMode::Duration => hours(segments) / parse_duration(&self.duration)?,
        };
        (rate.is_finite() && rate > 0.0).then_some(rate)
    }
}

fn hours(segments: &[(f64, f64)]) -> f64 {
    segments.iter().map(|(start, end)| end - start).sum()
}

fn period_count(horizon: f64) -> usize {
    (horizon / SCHEDULE_PERIOD_H).ceil().max(0.0) as usize
}

/// `mm:ss`, minutes not capped at an hour.
fn duration_text(seconds: f64) -> String {
    let total = seconds.max(0.0).round() as u64;
    format!("{:02}:{:02}", total / 60, total % 60)
}

/// `mm:ss`, `m:ss` or a plain number of seconds.
fn parse_duration(text: &str) -> Option<f64> {
    let text = text.trim();
    let seconds = match text.split_once(':') {
        Some((minutes, seconds)) => {
            let minutes: u64 = minutes.trim().parse().ok()?;
            let seconds: f64 = seconds.trim().parse().ok()?;
            (0.0..60.0).contains(&seconds).then_some(minutes as f64 * 60.0 + seconds)?
        }
        None => text.parse::<f64>().ok()?,
    };
    (seconds.is_finite() && seconds > 0.0).then_some(seconds)
}

pub(crate) fn draw_video_export_dialog(ui: &mut egui::Ui, editor: &mut EditorState, canvas_rect: egui::Rect, commands: &mut Vec<UiCommand>) {
    if editor.video_export_dialog.is_none() {
        return;
    }
    if !editor.is_schedule_animation() || editor.video_export_progress.is_some() {
        editor.video_export_dialog = None;
        if editor.video_export_progress.is_none() {
            editor.video_export_frame = None;
        }
        return;
    }
    let horizon = editor.schedule_animation_horizon_h.max(0.0);
    let pixels_per_point = ui.ctx().pixels_per_point();
    let size = (
        ((canvas_rect.width() * pixels_per_point) as u32) & !1,
        ((canvas_rect.height() * pixels_per_point) as u32) & !1,
    );
    let enabled = editor.schedule_animation_enabled;
    let dialog = editor.video_export_dialog.as_mut().expect("checked above");
    // The run may have been recalculated to a different length meanwhile.
    dialog.selected.resize(period_count(horizon), true);

    let mut open = true;
    let mut close = false;
    // Escape with a calendar or list open closes that, not the dialog. Read
    // before the contents are drawn, which is when the popup closes itself.
    let popup_open = egui::Popup::is_any_open(ui.ctx());
    DragableMenu::new("video_export_dialog", tr!("video-export-title"))
        .open(&mut open)
        .min_width(380.0)
        .default_pos(canvas_rect.left_top() + egui::vec2(16.0, 16.0))
        .show(ui.ctx(), |ui| {
            menu::menu_section(ui, tr!("video-export-range"));
            ui.horizontal(|ui| {
                ui.radio_value(&mut dialog.range, RangeMode::Periods, tr!("video-export-range-periods"));
                ui.radio_value(&mut dialog.range, RangeMode::Custom, tr!("video-export-range-custom"));
            });
            match dialog.range {
                RangeMode::Periods => draw_period_list(ui, dialog),
                RangeMode::Custom => {
                    time_field(ui, tr!("video-export-start"), &mut dialog.start);
                    time_field(ui, tr!("video-export-end"), &mut dialog.end);
                }
            }
            let segments = dialog.segments(horizon);

            menu::menu_section(ui, tr!("video-export-speed"));
            ui.radio_value(&mut dialog.speed, SpeedMode::Playback, tr!("video-export-speed-playback"));
            ui.add_enabled_ui(dialog.speed == SpeedMode::Playback, |ui| {
                MenuField::new(tr!("video-export-speed-rate")).show(ui, |ui, _, _| {
                    // The field's column runs right to left, so the row is
                    // added from its right-hand end: it reads "1 day = 15 s".
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut dialog.seconds).range(0.1..=86_400.0).speed(0.5).max_decimals(1).suffix(" s"));
                        ui.label("=");
                        let span = &mut dialog.span;
                        egui::ComboBox::from_id_salt("video_export_span").selected_text(span.label()).width(72.0).show_ui(ui, |ui| {
                            for option in PlaybackSpan::ALL {
                                ui.selectable_value(span, option, option.label());
                            }
                        });
                    });
                });
            });
            ui.radio_value(&mut dialog.speed, SpeedMode::Duration, tr!("video-export-speed-duration"));
            ui.add_enabled_ui(dialog.speed == SpeedMode::Duration, |ui| {
                MenuField::new(tr!("video-export-duration"))
                    .help_text(tr!("video-export-duration-help"))
                    .show(ui, |ui, height, width| {
                        let valid = parse_duration(&dialog.duration).is_some();
                        let mut edit = egui::TextEdit::singleline(&mut dialog.duration).hint_text("mm:ss");
                        if !valid {
                            edit = edit.text_color(ui.visuals().error_fg_color);
                        }
                        ui.add_sized([width, height], edit)
                    });
            });
            // Keep the length shown under Playback in step, so switching to
            // a length starts from the one the speed gives.
            if dialog.speed == SpeedMode::Playback
                && let Some(segments) = &segments
            {
                dialog.duration = duration_text(dialog.playback_seconds(segments));
            }

            menu::menu_section(ui, tr!("video-export-output"));
            MenuField::new(tr!("video-export-frame-rate")).show(ui, |ui, height, width| {
                ui.add_sized(
                    [width, height],
                    egui::DragValue::new(&mut dialog.fps).range(1.0..=MAX_FPS).speed(0.1).max_decimals(3).suffix(" fps"),
                )
            });
            // A named size, the viewport's own, or whatever is typed below.
            let named = PRESETS
                .iter()
                .map(|&(width, height, label)| {
                    (
                        (width, height),
                        tr!("video-export-size-named", name = label, width = width.to_string(), height = height.to_string()),
                    )
                })
                .chain(std::iter::once((
                    size,
                    tr!("video-export-size-viewport", width = size.0.to_string(), height = size.1.to_string()),
                )));
            let options: Vec<((u32, u32), String)> = named.collect();
            let mut chosen = (dialog.width, dialog.height);
            let selected = options
                .iter()
                .find(|(option, _)| *option == chosen)
                .map_or_else(|| tr!("video-export-size-custom"), |(_, label)| label.clone());
            menu::MenuFieldCombo::new(
                "video_export_size",
                tr!("video-export-size"),
                &mut chosen,
                selected,
                options.into_iter().map(|(option, label)| (option, label.into())),
            )
            .help_text(tr!("video-export-size-help"))
            .show(ui);
            (dialog.width, dialog.height) = chosen;
            menu::MenuFieldU32::new(tr!("video-export-width"), &mut dialog.width, 16..=MAX_SIDE).suffix(" px").show(ui);
            menu::MenuFieldU32::new(tr!("video-export-height"), &mut dialog.height, 16..=MAX_SIDE)
                .suffix(" px")
                .show(ui);

            if menu::menu_section_folding(ui, "video_export_display", tr!("video-export-display")) {
                menu::MenuFieldBool::new(tr!("video-export-destination-labels"), &mut dialog.destination_labels).show(ui);
                menu::MenuFieldBool::new(tr!("video-export-machine-labels"), &mut dialog.machine_labels).show(ui);
            }

            let request = segments.as_ref().filter(|segments| !segments.is_empty()).and_then(|segments| {
                let hours_per_second = dialog.hours_per_second(segments)?;
                Some(VideoExportRequest {
                    segments: segments.clone(),
                    hours_per_second,
                    fps: dialog.fps,
                    width: dialog.width,
                    height: dialog.height,
                    destination_labels: dialog.destination_labels,
                    machine_labels: dialog.machine_labels,
                })
            });
            let summary = match &request {
                Some(request) => {
                    let seconds = hours(&request.segments) / request.hours_per_second;
                    tr!("video-export-summary", length = duration_text(seconds), frames = request.frame_times().len().to_string())
                }
                None if segments.as_ref().is_some_and(Vec::is_empty) => tr!("video-export-nothing-selected"),
                None if segments.is_none() => tr!("video-export-invalid-range"),
                None => tr!("video-export-invalid-speed"),
            };
            menu::menu_note(ui, summary);
            menu::menu_note(ui, tr!("video-export-format-note"));
            menu::menu_actions(ui, |ui| {
                let ready = enabled && request.is_some();
                if ui.add(MenuButton::new(tr!("video-export-record")).primary().enabled(ready)).clicked()
                    && let Some(request) = request
                {
                    commands.push(UiCommand::ExportVideo(request));
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || (!popup_open && menu::dialog_cancelled(ui.ctx())) {
                    close = true;
                }
            });
        });
    // The viewport marks what the video will frame while the dialog is open.
    editor.video_export_frame = editor.video_export_dialog.as_ref().map(|dialog| (dialog.width, dialog.height));
    if close || !open {
        editor.video_export_dialog = None;
        editor.video_export_frame = None;
    }
}

/// A start or end instant: typed as every schedule page writes one, or - once
/// Day 1 has a date - picked from a calendar, keeping the time of day typed.
fn time_field(ui: &mut egui::Ui, label: String, text: &mut String) {
    MenuField::new(label).show(ui, |ui, height, width| {
        // The field's column runs right to left: the calendar is added
        // first so it sits at the right-hand end, after the text.
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            let start = clock_start();
            let parsed = parse_time(text, start);
            let mut picked = None;
            let button_width = if let Some(start) = start {
                let response = calendar_button(ui, height).on_hover_text(tr!("video-export-pick-date"));
                let current = parsed.and_then(|hours| start.checked_add_signed(chrono::Duration::days((hours / SCHEDULE_PERIOD_H).floor() as i64)));
                if let Some(date) = date_popup(&response, current) {
                    let day = (date - start).num_days().max(0) as f64;
                    let time_of_day = parsed.map_or(0.0, |hours| hours.rem_euclid(SCHEDULE_PERIOD_H));
                    picked = Some(editable_time(day * SCHEDULE_PERIOD_H + time_of_day));
                }
                height + 4.0
            } else {
                0.0
            };
            if let Some(picked) = picked {
                *text = picked;
            }
            let valid = parse_time(text, start).is_some();
            let mut edit = egui::TextEdit::singleline(text);
            if !valid {
                edit = edit.text_color(ui.visuals().error_fg_color);
            }
            ui.add_sized([width - button_width, height], edit);
        })
        .response
    });
}

/// A square button with a small calendar painted on it.
fn calendar_button(ui: &mut egui::Ui, side: f32) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(side, side), egui::Sense::click());
    let visuals = ui.style().interact(&response);
    ui.painter().rect(rect, GROUP_CORNER_RADIUS, visuals.bg_fill, visuals.bg_stroke, egui::StrokeKind::Inside);
    let color = visuals.fg_stroke.color;
    let page = egui::Rect::from_center_size(rect.center(), egui::vec2(side * 0.5, side * 0.46));
    let stroke = egui::Stroke::new(1.2, color);
    ui.painter().rect_stroke(page, 1.5, stroke, egui::StrokeKind::Inside);
    ui.painter()
        .rect_filled(egui::Rect::from_min_max(page.min, egui::pos2(page.max.x, page.min.y + page.height() * 0.3)), 1.5, color);
    for (column, row) in [(0, 0), (1, 0), (2, 0), (0, 1), (1, 1)] {
        let x = page.left() + page.width() * (0.25 + 0.25 * column as f32);
        let y = page.top() + page.height() * (0.55 + 0.25 * row as f32);
        ui.painter().circle_filled(egui::pos2(x, y), 0.9, color);
    }
    response
}

/// One row of the period list: a group of days, or a day.
enum Node {
    Group {
        key: String,
        label: String,
        children: Vec<Node>,
        periods: Vec<usize>,
    },
    Day(usize),
}

#[derive(Clone, Copy)]
enum Level {
    Year,
    Month,
    Week,
}

/// What groups a day at `level`: (sort key, label).
fn group_of(level: Level, period: usize, date: Option<NaiveDate>) -> (String, String) {
    match (level, date) {
        (Level::Year, Some(date)) => (format!("y{}", date.year()), date.year().to_string()),
        (Level::Month, Some(date)) => (
            format!("m{}-{}", date.year(), date.month()),
            tr!("video-export-month", month = month_name(date.month()), year = date.year().to_string()),
        ),
        (Level::Week, Some(date)) => {
            let week = date.iso_week();
            (format!("w{}-{}", week.year(), week.week()), tr!("video-export-week", week = week.week().to_string()))
        }
        // Without a start date a week is seven days from Day 1.
        _ => {
            let week = period / 7 + 1;
            (format!("w{week}"), tr!("video-export-week", week = week.to_string()))
        }
    }
}

/// The days grouped under only the levels that divide them.
fn build_tree(count: usize, start: Option<NaiveDate>) -> Vec<Node> {
    let date = |period: usize| start.and_then(|start| start.checked_add_signed(chrono::Duration::days(period as i64)));
    let distinct = |level: Level| {
        let mut keys: Vec<String> = (0..count).map(|period| group_of(level, period, date(period)).0).collect();
        keys.dedup();
        keys.len()
    };
    let levels: Vec<Level> = if start.is_some() {
        [Level::Year, Level::Month, Level::Week].into_iter().filter(|level| distinct(*level) > 1).collect()
    } else {
        [Level::Week].into_iter().filter(|level| distinct(*level) > 1).collect()
    };
    fn group(periods: &[usize], levels: &[Level], parent: &str, date: &dyn Fn(usize) -> Option<NaiveDate>) -> Vec<Node> {
        let Some((&level, rest)) = levels.split_first() else {
            return periods.iter().map(|&period| Node::Day(period)).collect();
        };
        let mut nodes = Vec::new();
        let mut at = 0;
        while at < periods.len() {
            let (key, label) = group_of(level, periods[at], date(periods[at]));
            let mut end = at + 1;
            while end < periods.len() && group_of(level, periods[end], date(periods[end])).0 == key {
                end += 1;
            }
            // A week split across two months is one row under each.
            let key = format!("{parent}/{key}");
            let members = periods[at..end].to_vec();
            let label = match level {
                Level::Week => {
                    let (first, last) = (members[0], members[members.len() - 1]);
                    tr!(
                        "video-export-week-range",
                        week = label,
                        from = short_day(first, date(first)),
                        to = short_day(last, date(last))
                    )
                }
                _ => label,
            };
            nodes.push(Node::Group {
                children: group(&members, rest, &key, date),
                key,
                label,
                periods: members,
            });
            at = end;
        }
        nodes
    }
    let periods: Vec<usize> = (0..count).collect();
    group(&periods, &levels, "", &date)
}

/// A day as a week row's range names it: "13/10", or "Day 15".
fn short_day(period: usize, date: Option<NaiveDate>) -> String {
    match date {
        Some(date) => format!("{:02}/{:02}", date.day(), date.month()),
        None => tr!("gantt-day", day = (period + 1).to_string()),
    }
}

fn tick_of(selected: &[bool], periods: &[usize]) -> Tick {
    let on = periods.iter().filter(|period| selected[**period]).count();
    Tick::of(on == periods.len() && !periods.is_empty(), on > 0)
}

fn draw_period_list(ui: &mut egui::Ui, dialog: &mut VideoExportDialog) {
    let count = dialog.selected.len();
    let all: Vec<usize> = (0..count).collect();
    if ChecklistRow::new(&tr!("video-export-select-all"), tick_of(&dialog.selected, &all)).show(ui).toggled {
        let on = tick_of(&dialog.selected, &all) != Tick::On;
        dialog.selected.iter_mut().for_each(|selected| *selected = on);
    }
    let tree = build_tree(count, clock_start());
    egui::ScrollArea::vertical().max_height(LIST_HEIGHT).auto_shrink([false, true]).show(ui, |ui| {
        draw_nodes(ui, dialog, &tree, 1);
    });
    menu::menu_note(ui, tr!("video-export-shift-click"));
}

fn draw_nodes(ui: &mut egui::Ui, dialog: &mut VideoExportDialog, nodes: &[Node], depth: usize) {
    for node in nodes {
        match node {
            Node::Group { key, label, children, periods } => {
                let open = dialog.expanded.contains(key);
                let tick = tick_of(&dialog.selected, periods);
                let response = ChecklistRow::new(label, tick).depth(depth).disclosure(Some(open)).show(ui);
                if response.expanded {
                    if open {
                        dialog.expanded.remove(key);
                    } else {
                        dialog.expanded.insert(key.clone());
                    }
                }
                if response.toggled {
                    let on = tick != Tick::On;
                    for period in periods {
                        dialog.selected[*period] = on;
                    }
                }
                if open {
                    draw_nodes(ui, dialog, children, depth + 1);
                }
            }
            Node::Day(period) => {
                let period = *period;
                let tick = if dialog.selected[period] { Tick::On } else { Tick::Off };
                if ChecklistRow::new(&day_label(period as u32), tick).depth(depth).show(ui).toggled {
                    let on = !dialog.selected[period];
                    let range = match dialog.anchor {
                        Some(anchor) if ui.input(|input| input.modifiers.shift) => anchor.min(period)..=anchor.max(period),
                        _ => period..=period,
                    };
                    for day in range {
                        dialog.selected[day] = on;
                    }
                    dialog.anchor = Some(period);
                }
            }
        }
    }
}
