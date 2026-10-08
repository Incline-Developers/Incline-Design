//! The Schedule's Charts: one chart per destination, made automatically from
//! the calculated schedule and laid along the Gantt's own timeline.
//!
//! The page shares the Gantt's view - zoom, pan, the time slider and the
//! Inspector - so moving between the two keeps the same stretch of time and
//! the same instant in front of the planner. What is charted follows from the
//! destinations, with nothing to set up:
//!
//! - a **stockpile**: what it holds, against its capacity, with the days its
//!   Mode stops building or reclaiming tinted behind;
//! - a **crusher**: its feed, hour by hour; and, for each tracked grade it has
//!   a target band for, the feed grade against that band, with the day's
//!   blend - which is what the target prices - as a bar across each day;
//! - a **dump**: what it has received so far.
//!
//! Each chart reads at a glance; the figures behind any instant are on hover.
//! A user-built reporting page is planned separately: this is deliberately
//! the fixed, automatic view, not a chart editor.

use crate::{
    i18n::tr,
    model::{
        ReserveFieldId,
        schedule::{
            CalendarPeriod, DestinationId, DestinationKind, SCHEDULE_PERIOD_H, SchedulePlan,
            destinations::DestinationView,
            result::{CalculatedSchedule, HourlyReceipts},
            stockpile_operation::PileMode,
        },
    },
    ui::{
        EditorState, UiProjectView,
        elements::{
            schedule_calendar::format_tonnes,
            schedule_gantt::{
                RECLAIM_COLOR, TimelineFrame, WORKING_COLOR, centred_note, draw_grid, draw_ruler, draw_time_slider, draw_timeline_page, instant_label, navigate, snap_slider,
            },
            schedule_inspector::{grade_number, grades_text},
        },
        fonts::bold,
        state::{GanttView, UiCommand},
    },
};

/// Height of a tonnes chart, and of a grade chart under its crusher.
const CHART_H: f32 = 96.0;
const GRADE_CHART_H: f32 = 120.0;
/// Room above a plot for its scale label, and below it before the next.
const PLOT_TOP: f32 = 18.0;
const PLOT_BOTTOM: f32 = 8.0;
/// Width of one sampled column. Two points is finer than the eye resolves on
/// a filled area and halves the work of one-point columns.
const COLUMN_W: f32 = 2.0;

const INVENTORY_COLOR: egui::Color32 = egui::Color32::from_rgb(0x4C, 0x9A, 0xE0);
const FEED_COLOR: egui::Color32 = egui::Color32::from_rgb(0x9B, 0x7F, 0xE0);
const RECEIVED_COLOR: egui::Color32 = egui::Color32::from_rgb(0xA8, 0x93, 0x70);
const BAND_COLOR: egui::Color32 = egui::Color32::from_rgb(0x3F, 0xB5, 0x8A);

#[derive(Clone, Copy, PartialEq)]
enum ChartKind {
    Inventory,
    Feed,
    /// The index into the result's tracked grades.
    Grade(usize, ReserveFieldId),
    Received,
    Trucks(crate::model::schedule::TruckClassId),
}

struct Chart {
    destination: DestinationId,
    name: String,
    kind: ChartKind,
    capacity: Option<f64>,
    height: f32,
}

/// The Charts page. Returns the rect it claimed.
pub(crate) fn draw_details(ui: &mut egui::Ui, editor: &mut EditorState, project: &UiProjectView, document: &crate::model::Document, commands: &mut Vec<UiCommand>) -> egui::Rect {
    let plan = project.schedule.clone();
    let fields: Vec<(ReserveFieldId, String)> = document.reserve_fields().iter().map(|field| (field.id, field.name.clone())).collect();
    draw_timeline_page(ui, editor, &plan, document, None, commands, |ui, rect, editor, destinations, _commands| {
        draw_canvas(ui, rect, editor, &plan, destinations, &fields);
    })
}

/// Which charts the destinations call for, in the order the Inspector lists
/// them: stockpiles, crushers each followed by their grade charts, dumps.
fn charts(plan: &SchedulePlan, destinations: &[DestinationView], schedule: &CalculatedSchedule, fields: &[(ReserveFieldId, String)]) -> Vec<Chart> {
    let mut charts = Vec::new();
    let chart = |view: &DestinationView, kind: ChartKind, height: f32| Chart {
        destination: view.id,
        name: view.name.clone(),
        kind,
        capacity: view.capacity_t.filter(|capacity| *capacity > 0.0),
        height,
    };
    for view in destinations.iter().filter(|view| view.kind == DestinationKind::Stockpile) {
        charts.push(chart(view, ChartKind::Inventory, CHART_H));
    }
    for view in destinations.iter().filter(|view| view.kind == DestinationKind::Crusher) {
        charts.push(chart(view, ChartKind::Feed, CHART_H));
        for (index, (field, _)) in schedule.grades.iter().enumerate() {
            let banded = plan
                .crusher_grade_calendars()
                .iter()
                .any(|calendar| calendar.destination == view.id && calendar.field == *field && (calendar.defaults.target.is_some() || !calendar.periods.is_empty()));
            if banded && fields.iter().any(|(id, _)| id == field) {
                charts.push(chart(view, ChartKind::Grade(index, *field), GRADE_CHART_H));
            }
        }
    }
    for view in destinations.iter().filter(|view| view.kind == DestinationKind::Dump) {
        charts.push(chart(view, ChartKind::Received, CHART_H));
    }
    for class in &plan.trucks().classes {
        charts.push(Chart {
            destination: DestinationId::Standalone(crate::model::schedule::StandaloneDestinationId(0)),
            name: class.name.clone(),
            kind: ChartKind::Trucks(class.id),
            capacity: None,
            height: CHART_H,
        });
    }
    charts
}

fn draw_canvas(ui: &mut egui::Ui, rect: egui::Rect, editor: &mut EditorState, plan: &SchedulePlan, destinations: &[DestinationView], fields: &[(ReserveFieldId, String)]) {
    // The zoom first, then the frame, as on the Gantt.
    let (over_canvas, rows_scroll, _) = navigate(ui, rect, TimelineFrame::new(editor.gantt, rect).body, editor, "charts_canvas");
    let frame = TimelineFrame::new(editor.gantt, rect);
    editor.schedule_charts_scroll += rows_scroll;

    let visuals = ui.visuals().clone();
    let rule = visuals.widgets.noninteractive.bg_stroke;
    let (surface, stripe) = crate::ui::widgets::tree_row_colors(ui);
    ui.painter().rect_filled(rect, 0.0, surface);
    ui.painter().rect_filled(frame.ruler, 0.0, visuals.widgets.noninteractive.bg_fill);
    ui.painter().rect_filled(frame.corner, 0.0, visuals.widgets.noninteractive.bg_fill);
    let interval = frame.interval(editor.gantt);
    draw_ruler(ui, frame.ruler, editor.gantt, interval, frame.day_band);

    let schedule = editor.schedule_result.clone();
    match &schedule {
        None => {
            draw_grid(ui, frame.body, editor.gantt, interval);
            centred_note(ui, frame.body, tr!("charts-no-schedule"));
        }
        Some(schedule) => {
            let charts = charts(plan, destinations, schedule, fields);
            if charts.is_empty() {
                draw_grid(ui, frame.body, editor.gantt, interval);
                centred_note(ui, frame.body, tr!("charts-no-destinations"));
            } else {
                let total: f32 = charts.iter().map(|chart| chart.height).sum();
                editor.schedule_charts_scroll = editor.schedule_charts_scroll.clamp(0.0, (total - frame.body.height()).max(0.0));
                let mut top = frame.body.top() - editor.schedule_charts_scroll;
                for (index, chart) in charts.iter().enumerate() {
                    let row = egui::Rect::from_min_size(egui::pos2(rect.left(), top), egui::vec2(rect.width(), chart.height));
                    top += chart.height;
                    if row.bottom() < frame.body.top() || row.top() > frame.body.bottom() {
                        continue;
                    }
                    let clip = frame.body.union(frame.header).intersect(rect);
                    let mut child = ui.new_child(egui::UiBuilder::new().id_salt(("chart", index)).max_rect(row));
                    child.set_clip_rect(clip.intersect(row));
                    if index % 2 == 1 {
                        child.painter().rect_filled(row, 0.0, stripe);
                    }
                    let plot = egui::Rect::from_min_max(
                        egui::pos2(frame.body.left(), row.top() + PLOT_TOP),
                        egui::pos2(frame.body.right(), row.bottom() - PLOT_BOTTOM),
                    );
                    let header = egui::Rect::from_min_max(row.min, egui::pos2(frame.body.left(), row.bottom()));
                    draw_grid(
                        &child,
                        egui::Rect::from_min_max(egui::pos2(frame.body.left(), row.top()), egui::pos2(frame.body.right(), row.bottom())),
                        editor.gantt,
                        interval,
                    );
                    draw_chart(&mut child, chart, plot, header, editor, plan, schedule, fields);
                    child.painter().line_segment([row.left_bottom(), row.right_bottom()], rule);
                }
            }
        }
    }

    let painter = ui.painter();
    painter.line_segment([frame.ruler.left_bottom(), frame.ruler.right_bottom()], rule);
    painter.line_segment([frame.corner.right_top(), frame.header.right_bottom()], rule);
    painter.rect_stroke(rect, 0.0, rule, egui::StrokeKind::Inside);
    draw_time_slider(ui, frame.ruler, frame.body, editor, interval, over_canvas);
}

/// The hours `[from, to)` of one sampled column, held inside what the
/// calculation covers, or `None` when the column lies outside it.
fn column_hours(view: GanttView, plot: egui::Rect, x: f32, end_h: f64) -> Option<(f64, f64)> {
    let from = view.seconds_at(x, plot.left(), plot.width()) / GanttView::HOUR;
    let to = view.seconds_at(x + COLUMN_W, plot.left(), plot.width()) / GanttView::HOUR;
    let (from, to) = (from.max(0.0), to.min(end_h));
    (to > from).then_some((from, to))
}

/// The tonnes and contained quantity of one grade received over `[from, to)`,
/// from the hourly series, each hour counted for the part inside.
fn sum_over(series: &HourlyReceipts, grade: Option<usize>, from: f64, to: f64) -> (f64, f64) {
    let mut tonnes = 0.0;
    let mut contained = 0.0;
    let first = from.floor().max(0.0) as usize;
    let last = (to.ceil().max(0.0) as usize).min(series.tonnes.len());
    for hour in first..last {
        let share = (to.min(hour as f64 + 1.0) - from.max(hour as f64)).clamp(0.0, 1.0);
        tonnes += series.tonnes[hour] * share;
        if let Some(grade) = grade {
            contained += series.contained.get(grade).map_or(0.0, |values| values[hour]) * share;
        }
    }
    (tonnes, contained)
}

fn y_of(plot: egui::Rect, value: f64, low: f64, high: f64) -> f32 {
    let span = (high - low).max(f64::EPSILON);
    plot.bottom() - (((value - low) / span).clamp(0.0, 1.0) as f32) * plot.height()
}

/// A scale figure, on a faint backing so a line drawn through it does not
/// make it unreadable.
fn scale_label(ui: &egui::Ui, pos: egui::Pos2, align: egui::Align2, text: String) {
    let color = ui.visuals().weak_text_color();
    let galley = ui.painter().layout_no_wrap(text, egui::FontId::proportional(11.0), color);
    let rect = align.anchor_size(pos, galley.size());
    ui.painter()
        .rect_filled(rect.expand2(egui::vec2(3.0, 0.0)), 2.0, ui.visuals().extreme_bg_color.gamma_multiply(0.75));
    ui.painter().galley(rect.min, galley, color);
}

#[allow(clippy::too_many_arguments, reason = "one chart's whole context, passed once")]
fn draw_chart(
    ui: &mut egui::Ui,
    chart: &Chart,
    plot: egui::Rect,
    header: egui::Rect,
    editor: &mut EditorState,
    plan: &SchedulePlan,
    schedule: &CalculatedSchedule,
    fields: &[(ReserveFieldId, String)],
) {
    let view = editor.gantt;
    let end_h = schedule.requested_end_h;
    let slider_h = editor.schedule_time_h;
    let grade_names: Vec<String> = schedule
        .grades
        .iter()
        .map(|(field, _)| fields.iter().find(|(id, _)| id == field).map_or_else(|| field.0.to_string(), |(_, name)| name.clone()))
        .collect();
    if let ChartKind::Trucks(id) = chart.kind {
        draw_trucks(ui, header, plot, view, plan, schedule, id, slider_h);
        return;
    }
    let receipts = schedule.hourly_receipts(chart.destination);
    let empty = HourlyReceipts::default();
    let receipts = receipts.unwrap_or(&empty);
    let painter = ui.painter().clone();

    // What the chart says at the slider, for the header.
    let (kind_label, at_slider) = match chart.kind {
        ChartKind::Trucks(_) => unreachable!("truck chart drawn above"),
        ChartKind::Inventory => (
            tr!("charts-inventory"),
            schedule
                .inventory_at(chart.destination, slider_h)
                .map(|(tonnes, _)| tr!("inspector-tonnes", tonnes = format_tonnes(tonnes.round()))),
        ),
        ChartKind::Feed => (
            tr!("charts-feed"),
            (slider_h < end_h).then(|| {
                let hour = slider_h.floor();
                tr!("inspector-rate", rate = format_tonnes(sum_over(receipts, None, hour, hour + 1.0).0.round()))
            }),
        ),
        // The day's blend, which is what the band prices.
        ChartKind::Grade(_, field) => (
            tr!(
                "charts-grade",
                grade = fields.iter().find(|(id, _)| *id == field).map(|(_, name)| name.clone()).unwrap_or_default()
            ),
            {
                let day = (slider_h / SCHEDULE_PERIOD_H).floor() as u32;
                schedule
                    .grade_targets
                    .iter()
                    .find(|target| target.period == day && target.specification.destination == chart.destination && target.specification.field == field && target.tonnes > 1e-6)
                    .filter(|_| slider_h < end_h)
                    .map(|target| tr!("charts-grade-day", grade = grade_number(target.contained / target.tonnes)))
            },
        ),
        ChartKind::Received => (
            tr!("charts-received"),
            Some(tr!(
                "inspector-tonnes",
                tonnes = format_tonnes(sum_over(receipts, None, 0.0, slider_h.min(end_h)).0.round())
            )),
        ),
    };
    let text = ui.visuals().text_color();
    let weak = ui.visuals().weak_text_color();
    let strong = ui.visuals().strong_text_color();
    let name = egui::WidgetText::from(bold(&chart.name)).into_galley(ui, Some(egui::TextWrapMode::Truncate), header.width() - 16.0, egui::TextStyle::Body);
    painter.galley(egui::pos2(header.left() + 8.0, header.top() + 8.0), name, text);
    painter.text(
        egui::pos2(header.left() + 8.0, header.top() + 28.0),
        egui::Align2::LEFT_TOP,
        kind_label,
        egui::FontId::proportional(12.0),
        weak,
    );
    if let Some(value) = at_slider {
        painter.text(
            egui::pos2(header.left() + 8.0, header.top() + 46.0),
            egui::Align2::LEFT_TOP,
            value,
            egui::FontId::proportional(15.0),
            strong,
        );
    }
    // How to read the chart, on the header rather than drawn as a legend.
    let help = match chart.kind {
        ChartKind::Trucks(_) => unreachable!("truck chart drawn above"),
        ChartKind::Inventory => tr!("charts-inventory-help"),
        ChartKind::Feed => tr!("charts-feed-help"),
        ChartKind::Grade(..) => tr!("charts-grade-help"),
        ChartKind::Received => tr!("charts-received-help"),
    };
    ui.interact(header, ui.id().with("header"), egui::Sense::hover()).on_hover_text(help);

    let columns = ((plot.width() / COLUMN_W).ceil().max(0.0)) as usize;
    let xs = (0..columns).map(|index| plot.left() + index as f32 * COLUMN_W);
    let mut hover_lines: Option<Vec<String>> = None;
    let pointer = ui
        .input(|input| input.pointer.hover_pos())
        .filter(|pos| plot.expand2(egui::vec2(0.0, PLOT_TOP)).contains(*pos) && ui.clip_rect().contains(*pos));
    let pointer_h = pointer.map(|pos| view.seconds_at(pos.x, plot.left(), plot.width()) / GanttView::HOUR);

    match chart.kind {
        ChartKind::Trucks(_) => unreachable!("truck chart drawn above"),
        ChartKind::Inventory => {
            let peak = schedule.inventory_peak(chart.destination);
            let high = chart.capacity.unwrap_or(0.0).max(peak).max(1.0) * 1.08;
            // The days a Mode holds the pile, tinted behind.
            let operation = plan.stockpile_operation(chart.destination);
            let first_day = (view.start_seconds / GanttView::DAY).floor().max(0.0) as u32;
            let last_day = ((view.end_seconds() / GanttView::DAY).ceil().max(0.0) as u32).min((end_h / SCHEDULE_PERIOD_H).ceil() as u32);
            for day in first_day..last_day {
                let tint = match operation.mode_at(CalendarPeriod(day)) {
                    PileMode::BuildAndReclaim => continue,
                    PileMode::BuildOnly => WORKING_COLOR,
                    PileMode::ReclaimOnly => RECLAIM_COLOR,
                    PileMode::Off => ui.visuals().weak_text_color(),
                };
                let left = view.x_of(f64::from(day) * GanttView::DAY, plot.left(), plot.width()).max(plot.left());
                let right = view.x_of(f64::from(day + 1) * GanttView::DAY, plot.left(), plot.width()).min(plot.right());
                if right > left {
                    painter.rect_filled(
                        egui::Rect::from_min_max(egui::pos2(left, plot.top()), egui::pos2(right, plot.bottom())),
                        0.0,
                        tint.gamma_multiply(0.12),
                    );
                }
            }
            let mut mesh = egui::Mesh::default();
            let mut line = Vec::new();
            for x in xs {
                let Some((from, to)) = column_hours(view, plot, x, end_h) else { continue };
                let Some((tonnes, _)) = schedule.inventory_at(chart.destination, (from + to) / 2.0) else {
                    continue;
                };
                let y = y_of(plot, tonnes, 0.0, high);
                mesh.add_colored_rect(
                    egui::Rect::from_min_max(egui::pos2(x, y), egui::pos2(x + COLUMN_W, plot.bottom())),
                    INVENTORY_COLOR.gamma_multiply(0.35),
                );
                line.push(egui::pos2(x + COLUMN_W / 2.0, y));
            }
            painter.add(egui::Shape::mesh(mesh));
            painter.add(egui::Shape::line(line, egui::Stroke::new(1.5, INVENTORY_COLOR)));
            if let Some(capacity) = chart.capacity {
                let y = y_of(plot, capacity, 0.0, high);
                painter.add(egui::Shape::dashed_line(
                    &[egui::pos2(plot.left(), y), egui::pos2(plot.right(), y)],
                    egui::Stroke::new(1.0, weak),
                    6.0,
                    4.0,
                ));
                scale_label(
                    ui,
                    egui::pos2(plot.left() + 4.0, y - 2.0),
                    egui::Align2::LEFT_BOTTOM,
                    tr!("charts-capacity", tonnes = format_tonnes(capacity.round())),
                );
            } else {
                scale_label(
                    ui,
                    egui::pos2(plot.left() + 4.0, plot.top() - 2.0),
                    egui::Align2::LEFT_BOTTOM,
                    tr!("inspector-tonnes", tonnes = format_tonnes(peak.round())),
                );
            }
            if let Some(hour) = pointer_h.filter(|hour| (0.0..end_h).contains(hour)) {
                let mut lines = vec![bold_line(&chart.name, hour)];
                if let Some((tonnes, contained)) = schedule.inventory_at(chart.destination, hour) {
                    lines.push(tr!("inspector-tonnes", tonnes = format_tonnes(tonnes.round())));
                    if let Some(grades) = grades_text(&grade_names, tonnes, &contained) {
                        lines.push(grades);
                    }
                }
                let mode = operation.mode_at(CalendarPeriod((hour / SCHEDULE_PERIOD_H).floor() as u32));
                if mode != PileMode::default() {
                    lines.push(tr!("charts-mode", mode = mode.label()));
                }
                hover_lines = Some(lines);
            }
        }
        ChartKind::Feed | ChartKind::Received if receipts.tonnes.iter().all(|tonnes| *tonnes <= 1e-6) => {
            // A flat line at zero with a made-up scale says less than this.
            painter.text(
                plot.center(),
                egui::Align2::CENTER_CENTER,
                tr!("charts-nothing-received"),
                egui::FontId::proportional(12.0),
                weak,
            );
        }
        ChartKind::Feed => {
            let high = receipts.tonnes.iter().copied().fold(0.0, f64::max).max(1.0) * 1.08;
            let mut mesh = egui::Mesh::default();
            for x in xs {
                let Some((from, to)) = column_hours(view, plot, x, end_h) else { continue };
                let rate = sum_over(receipts, None, from, to).0 / (to - from);
                if rate <= 1e-6 {
                    continue;
                }
                let y = y_of(plot, rate, 0.0, high);
                mesh.add_colored_rect(
                    egui::Rect::from_min_max(egui::pos2(x, y), egui::pos2(x + COLUMN_W, plot.bottom())),
                    FEED_COLOR.gamma_multiply(0.75),
                );
            }
            painter.add(egui::Shape::mesh(mesh));
            scale_label(
                ui,
                egui::pos2(plot.left() + 4.0, plot.top() - 2.0),
                egui::Align2::LEFT_BOTTOM,
                tr!("inspector-rate", rate = format_tonnes((high / 1.08).round())),
            );
            if let Some(hour) = pointer_h.filter(|hour| (0.0..end_h).contains(hour)) {
                let start = hour.floor();
                let day = (hour / SCHEDULE_PERIOD_H).floor();
                let day_start = day * SCHEDULE_PERIOD_H;
                let (day_tonnes, _) = sum_over(receipts, None, day_start, (day_start + SCHEDULE_PERIOD_H).min(end_h));
                let limit = match chart.destination {
                    DestinationId::Standalone(id) => plan.routing().crusher(id).and_then(|calendar| calendar.limit_at(CalendarPeriod(day as u32))),
                    _ => None,
                };
                let mut lines = vec![
                    bold_line(&chart.name, hour),
                    tr!("charts-feed-hour", rate = format_tonnes(sum_over(receipts, None, start, start + 1.0).0.round())),
                ];
                lines.push(match limit {
                    Some(limit) => tr!("charts-feed-day-of", tonnes = format_tonnes(day_tonnes.round()), limit = format_tonnes(limit.round())),
                    None => tr!("charts-feed-day", tonnes = format_tonnes(day_tonnes.round())),
                });
                hover_lines = Some(lines);
            }
        }
        ChartKind::Grade(grade, field) => {
            let calendar = plan.crusher_grade_calendar(chart.destination, field);
            let days = (end_h / SCHEDULE_PERIOD_H).ceil().max(0.0) as u32;
            // A scale that holds still while the view pans, set by what the
            // chart is about: every day's band and every day's blend. Single
            // hours stray much further than a day's blend does, and scaling
            // to them squeezes the band to a line; they clip at the edges
            // instead, and the hover still gives their value.
            let mut low = f64::INFINITY;
            let mut high = f64::NEG_INFINITY;
            for day in 0..days {
                let values = calendar.resolved(CalendarPeriod(day));
                for value in [values.lower, values.target, values.upper].into_iter().flatten() {
                    low = low.min(value);
                    high = high.max(value);
                }
            }
            for target in schedule
                .grade_targets
                .iter()
                .filter(|target| target.specification.destination == chart.destination && target.specification.field == field && target.tonnes > 1e-6)
            {
                let value = target.contained / target.tonnes;
                low = low.min(value);
                high = high.max(value);
            }
            if !low.is_finite() || !high.is_finite() {
                for (hour, tonnes) in receipts.tonnes.iter().enumerate() {
                    if *tonnes > 1e-6 {
                        let value = receipts.contained.get(grade).map_or(0.0, |values| values[hour]) / tonnes;
                        low = low.min(value);
                        high = high.max(value);
                    }
                }
            }
            if !low.is_finite() || !high.is_finite() {
                low = 0.0;
                high = 1.0;
            }
            let pad = ((high - low) * 0.35).max(high.abs() * 0.01).max(1e-6);
            let (low, high) = (low - pad, high + pad);

            let first_day = (view.start_seconds / GanttView::DAY).floor().max(0.0) as u32;
            let last_day = ((view.end_seconds() / GanttView::DAY).ceil().max(0.0) as u32).min(days);
            for day in first_day..last_day {
                let values = calendar.resolved(CalendarPeriod(day));
                let left = view.x_of(f64::from(day) * GanttView::DAY, plot.left(), plot.width()).max(plot.left());
                let right = view
                    .x_of((f64::from(day + 1) * GanttView::DAY).min(end_h * GanttView::HOUR), plot.left(), plot.width())
                    .min(plot.right());
                if right <= left {
                    continue;
                }
                if values.lower.is_some() || values.upper.is_some() {
                    let top = values.upper.map_or(plot.top(), |upper| y_of(plot, upper, low, high));
                    let bottom = values.lower.map_or(plot.bottom(), |lower| y_of(plot, lower, low, high));
                    painter.rect_filled(
                        egui::Rect::from_min_max(egui::pos2(left, top), egui::pos2(right, bottom)),
                        0.0,
                        BAND_COLOR.gamma_multiply(0.16),
                    );
                }
                if let Some(target) = values.target {
                    let y = y_of(plot, target, low, high);
                    painter.line_segment([egui::pos2(left, y), egui::pos2(right, y)], egui::Stroke::new(1.0, BAND_COLOR.gamma_multiply(0.8)));
                }
            }
            // The feed's grade, broken where nothing was fed.
            let mut run: Vec<egui::Pos2> = Vec::new();
            let stroke = egui::Stroke::new(1.25, FEED_COLOR);
            for x in xs {
                let value = column_hours(view, plot, x, end_h).and_then(|(from, to)| {
                    let (tonnes, contained) = sum_over(receipts, Some(grade), from, to);
                    (tonnes > 1e-6).then(|| contained / tonnes)
                });
                match value {
                    Some(value) => run.push(egui::pos2(x + COLUMN_W / 2.0, y_of(plot, value, low, high))),
                    None if run.len() > 1 => {
                        painter.add(egui::Shape::line(std::mem::take(&mut run), stroke));
                    }
                    None => run.clear(),
                }
            }
            if run.len() > 1 {
                painter.add(egui::Shape::line(run, stroke));
            }
            // The day's blend, which is what the band prices.
            for target in schedule
                .grade_targets
                .iter()
                .filter(|target| target.specification.destination == chart.destination && target.specification.field == field && target.tonnes > 1e-6)
            {
                let value = target.contained / target.tonnes;
                let spec = &target.specification;
                let outside = spec.lower.is_some_and(|lower| value < lower - 1e-9) || spec.upper.is_some_and(|upper| value > upper + 1e-9);
                let left = view.x_of(f64::from(target.period) * GanttView::DAY, plot.left(), plot.width());
                let right = view.x_of((f64::from(target.period + 1) * GanttView::DAY).min(end_h * GanttView::HOUR), plot.left(), plot.width());
                if right < plot.left() || left > plot.right() {
                    continue;
                }
                let y = y_of(plot, value, low, high);
                let color = if outside { ui.visuals().warn_fg_color } else { strong };
                painter.line_segment(
                    [egui::pos2(left.max(plot.left()) + 2.0, y), egui::pos2(right.min(plot.right()) - 2.0, y)],
                    egui::Stroke::new(2.5, color),
                );
            }
            scale_label(ui, egui::pos2(plot.left() + 4.0, plot.top() - 2.0), egui::Align2::LEFT_BOTTOM, grade_number(high));
            scale_label(ui, egui::pos2(plot.left() + 4.0, plot.bottom() - 2.0), egui::Align2::LEFT_BOTTOM, grade_number(low));
            if let Some(hour) = pointer_h.filter(|hour| (0.0..end_h).contains(hour)) {
                let name = grade_names.get(grade).cloned().unwrap_or_default();
                let start = hour.floor();
                let day = (hour / SCHEDULE_PERIOD_H).floor() as u32;
                let mut lines = vec![bold_line(&chart.name, hour)];
                let (tonnes, contained) = sum_over(receipts, Some(grade), start, start + 1.0);
                lines.push(if tonnes > 1e-6 {
                    tr!("charts-grade-hour", grade = format!("{name} {}", grade_number(contained / tonnes)))
                } else {
                    tr!("inspector-not-fed")
                });
                if let Some(target) = schedule
                    .grade_targets
                    .iter()
                    .find(|target| target.period == day && target.specification.destination == chart.destination && target.specification.field == field && target.tonnes > 1e-6)
                {
                    let spec = &target.specification;
                    let bound = |value: Option<f64>| value.map_or_else(|| "—".to_owned(), grade_number);
                    lines.push(tr!(
                        "inspector-crusher-day-grade",
                        grade = format!("{name} {}", grade_number(target.contained / target.tonnes)),
                        lower = bound(spec.lower),
                        target = grade_number(spec.target),
                        upper = bound(spec.upper)
                    ));
                }
                hover_lines = Some(lines);
            }
        }
        ChartKind::Received => {
            let total = receipts.tonnes.iter().sum::<f64>().max(1.0);
            let high = total * 1.08;
            let mut mesh = egui::Mesh::default();
            let mut line = Vec::new();
            // Running total from hour zero to each column's end; the columns
            // are visited in time order, so the sum is carried along rather
            // than recounted from zero for each.
            let mut carried = 0.0;
            let mut carried_to = 0.0;
            for x in xs {
                let Some((_, to)) = column_hours(view, plot, x, end_h) else { continue };
                carried += sum_over(receipts, None, carried_to, to).0;
                carried_to = to;
                let y = y_of(plot, carried, 0.0, high);
                mesh.add_colored_rect(
                    egui::Rect::from_min_max(egui::pos2(x, y), egui::pos2(x + COLUMN_W, plot.bottom())),
                    RECEIVED_COLOR.gamma_multiply(0.3),
                );
                line.push(egui::pos2(x + COLUMN_W / 2.0, y));
            }
            painter.add(egui::Shape::mesh(mesh));
            painter.add(egui::Shape::line(line, egui::Stroke::new(1.5, RECEIVED_COLOR)));
            scale_label(
                ui,
                egui::pos2(plot.left() + 4.0, plot.top() - 2.0),
                egui::Align2::LEFT_BOTTOM,
                tr!("inspector-tonnes", tonnes = format_tonnes(total.round())),
            );
            if let Some(hour) = pointer_h.filter(|hour| (0.0..end_h).contains(hour)) {
                hover_lines = Some(vec![
                    bold_line(&chart.name, hour),
                    tr!("inspector-dump-to-date", tonnes = format_tonnes(sum_over(receipts, None, 0.0, hour).0.round())),
                ]);
            }
        }
    }

    // Pointing at a chart marks the instant across it; pressing moves the
    // time slider there, as pressing the ruler does.
    let response = ui.interact(plot, ui.id().with("plot"), egui::Sense::click_and_drag());
    if (response.is_pointer_button_down_on() || response.clicked())
        && let Some(pos) = response.interact_pointer_pos()
    {
        let interval = view.minor_interval(plot.width(), 72.0);
        editor.schedule_time_h = snap_slider(view.seconds_at(pos.x, plot.left(), plot.width()), interval) / GanttView::HOUR;
    }
    if let (Some(pos), Some(lines)) = (pointer, hover_lines) {
        painter.line_segment([egui::pos2(pos.x, plot.top()), egui::pos2(pos.x, plot.bottom())], egui::Stroke::new(1.0, weak));
        response.on_hover_ui_at_pointer(|ui| {
            for (index, line) in lines.iter().enumerate() {
                if index == 0 {
                    ui.label(bold(line));
                } else {
                    ui.label(line);
                }
            }
        });
    }
}

fn bold_line(name: &str, hour: f64) -> String {
    format!("{name} · {}", instant_label(hour * GanttView::HOUR))
}

#[allow(clippy::too_many_arguments, reason = "chart geometry and shared timeline inputs")]
fn draw_trucks(
    ui: &mut egui::Ui,
    header: egui::Rect,
    plot: egui::Rect,
    view: GanttView,
    plan: &SchedulePlan,
    schedule: &CalculatedSchedule,
    id: crate::model::schedule::TruckClassId,
    slider: f64,
) {
    let Some(class) = plan.trucks().class(id) else { return };
    let painter = ui.painter().with_clip_rect(plot);
    ui.painter().text(
        header.left_top() + egui::vec2(8.0, 8.0),
        egui::Align2::LEFT_TOP,
        &class.name,
        egui::FontId::proportional(14.0),
        ui.visuals().text_color(),
    );
    ui.painter().text(
        header.left_top() + egui::vec2(8.0, 28.0),
        egui::Align2::LEFT_TOP,
        tr!("inspector-trucks"),
        egui::FontId::proportional(12.0),
        ui.visuals().weak_text_color(),
    );
    let fleet = class
        .calendar
        .values_at(CalendarPeriod((slider / SCHEDULE_PERIOD_H).floor().max(0.0) as u32))
        .effective_units();
    ui.painter().text(
        header.left_top() + egui::vec2(8.0, 46.0),
        egui::Align2::LEFT_TOP,
        tr!(
            "inspector-trucks-in-use",
            busy = format!("{:.1}", schedule.trucks_in_use(id, slider)),
            fleet = format!("{fleet:.1}")
        ),
        egui::FontId::proportional(13.0),
        ui.visuals().text_color(),
    );
    let from = (view.start_seconds / GanttView::HOUR).max(0.0);
    let to = (view.end_seconds() / GanttView::HOUR).min(schedule.requested_end_h);
    let knots = schedule.truck_use_knots(id);
    let mut times = vec![from, to];
    times.extend(knots.iter().map(|(h, _)| *h).filter(|h| *h > from && *h < to));
    let first_day = (from / SCHEDULE_PERIOD_H).floor().max(0.0) as u32;
    let last_day = (to / SCHEDULE_PERIOD_H).ceil().max(0.0) as u32;
    times.extend((first_day..=last_day).map(|d| f64::from(d) * SCHEDULE_PERIOD_H).filter(|h| *h > from && *h < to));
    times.sort_by(f64::total_cmp);
    times.dedup();
    let samples: Vec<_> = times
        .iter()
        .map(|&at| {
            (
                view.x_of(at * GanttView::HOUR, plot.left(), plot.width()),
                schedule.trucks_in_use(id, at),
                class.calendar.values_at(CalendarPeriod((at / SCHEDULE_PERIOD_H).floor() as u32)).effective_units(),
            )
        })
        .collect();
    let high = samples.iter().map(|(_, busy, fleet)| busy.max(*fleet)).fold(1.0, f64::max) * 1.1;
    for pair in samples.windows(2) {
        let [(x0, b0, f0), (x1, b1, f1)] = [pair[0], pair[1]];
        for (before, after, color, width) in [(b0, b1, INVENTORY_COLOR, 2.0), (f0, f1, ui.visuals().weak_text_color(), 1.0)] {
            let y0 = y_of(plot, before, 0.0, high);
            let y1 = y_of(plot, after, 0.0, high);
            painter.line_segment([egui::pos2(x0, y0), egui::pos2(x1, y0)], egui::Stroke::new(width, color));
            painter.line_segment([egui::pos2(x1, y0), egui::pos2(x1, y1)], egui::Stroke::new(width, color));
        }
    }
    if let Some(pointer) = ui.input(|i| i.pointer.hover_pos()).filter(|p| plot.contains(*p)) {
        let hour = view.seconds_at(pointer.x, plot.left(), plot.width()) / GanttView::HOUR;
        let fleet = class
            .calendar
            .values_at(CalendarPeriod((hour / SCHEDULE_PERIOD_H).floor().max(0.0) as u32))
            .effective_units();
        ui.interact(plot, ui.id().with("haul_truck_hover"), egui::Sense::hover()).on_hover_text(tr!(
            "inspector-trucks-in-use",
            busy = format!("{:.1}", schedule.trucks_in_use(id, hour)),
            fleet = format!("{fleet:.1}")
        ));
    }
}
