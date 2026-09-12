//! The Schedule workspace's Gantt: one timeline row per loader agent, along
//! elapsed project time.
//!
//! Stage 1 draws the rows, the ruler and the navigation. It deliberately
//! draws *no* bars: a schedule is calculated from sequences and tonnages that
//! do not exist yet, and a painted rectangle here would look exactly like one
//! that had been.
//!
//! Time is elapsed project time in seconds - zero is `Day 1, 00:00`, not a
//! calendar date and not the computer clock. Everything is computed in `f64`
//! seconds and converted to points against whatever rect the timeline gets
//! this frame, so nothing drifts across a resize or a DPI change. Only the
//! ticks and rows actually on screen are visited, so scrolling to day 300
//! costs the same as day 1.

use crate::{
    i18n::{tr, tr_format},
    ui::{EditorState, UiProjectView, chrome, fonts::bold, state::GanttView, widgets::toolbar::GROUP_CORNER_RADIUS},
};

/// Width of the fixed agent-name column. Wide enough for a machine name and
/// its class beneath it; never scrolled, so a horizontal pan leaves it alone.
const HEADER_WIDTH: f32 = 200.0;
/// Height of one agent's row.
const ROW_HEIGHT: f32 = 34.0;
/// Height of one band of the time ruler. Two bands are drawn when the minor
/// ticks are finer than a day, so the days above can group the hours below.
const RULER_BAND: f32 = 20.0;
/// Closest two labelled ticks are allowed to sit. Interval selection works
/// back from this, which is what keeps labels legible at any pane width.
const MIN_TICK_SPACING: f32 = 72.0;
/// How much one zoom-button press changes the visible span.
const ZOOM_STEP: f64 = 1.5;

/// Write one elapsed-seconds instant as a day-and-time label.
///
/// Day 1 starts at zero, so the first day of a schedule reads `Day 1` rather
/// than `Day 0` - which is how a mine plan is written and read.
fn day_of(seconds: f64) -> i64 {
    (seconds / GanttView::DAY).floor() as i64 + 1
}

fn time_of(seconds: f64) -> String {
    let into_day = seconds - (seconds / GanttView::DAY).floor() * GanttView::DAY;
    let minutes = (into_day / 60.0).round() as i64;
    format!("{:02}:{:02}", (minutes / 60).clamp(0, 23), minutes % 60)
}

fn instant_label(seconds: f64) -> String {
    tr!("gantt-day-time", day = day_of(seconds).to_string(), time = time_of(seconds))
}

/// The Gantt page. Returns the rect it claimed, for the caller to round off
/// as one chrome region.
pub(crate) fn draw_details(ui: &mut egui::Ui, editor: &mut EditorState, project: &UiProjectView) -> egui::Rect {
    egui::CentralPanel::default()
        .frame(chrome::region_frame(ui))
        .show(ui, |ui| {
            let available = ui.available_rect_before_wrap();
            let toolbar_height = ui.spacing().interact_size.y + 8.0;
            let toolbar = egui::Rect::from_min_size(available.min, egui::vec2(available.width(), toolbar_height.min(available.height())));
            let canvas = egui::Rect::from_min_max(egui::pos2(available.left(), toolbar.bottom()), available.max);
            draw_toolbar(ui, toolbar, editor);
            if canvas.is_positive() {
                draw_canvas(ui, canvas, editor, project);
            }
            ui.allocate_rect(available, egui::Sense::hover());
        })
        .response
        .rect
}

/// Zoom controls, Reset View, and what window of project time is on screen.
fn draw_toolbar(ui: &mut egui::Ui, rect: egui::Rect, editor: &mut EditorState) {
    let mut child = ui.new_child(egui::UiBuilder::new().id_salt("gantt_toolbar").max_rect(rect));
    child.set_clip_rect(child.clip_rect().intersect(rect));
    child.horizontal_centered(|ui| {
        ui.add_space(4.0);
        let button = |ui: &mut egui::Ui, label: &str, tooltip: String| ui.add(egui::Button::new(label).corner_radius(GROUP_CORNER_RADIUS)).on_hover_text(tooltip);
        if button(ui, "−", tr!("gantt-zoom-out")).clicked() {
            editor.gantt.zoom_at(1.0 / ZOOM_STEP, 0.5);
        }
        if button(ui, "+", tr!("gantt-zoom-in")).clicked() {
            editor.gantt.zoom_at(ZOOM_STEP, 0.5);
        }
        if ui.add(egui::Button::new(tr!("gantt-reset-view")).corner_radius(GROUP_CORNER_RADIUS)).clicked() {
            editor.gantt.reset();
        }
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new(tr!(
                "gantt-range",
                from = instant_label(editor.gantt.start_seconds),
                to = instant_label(editor.gantt.end_seconds())
            ))
            .weak(),
        );
    });
}

/// The header column, the ruler and the rows, plus the navigation over them.
fn draw_canvas(ui: &mut egui::Ui, rect: egui::Rect, editor: &mut EditorState, project: &UiProjectView) {
    let plan = &project.schedule;
    let header_width = HEADER_WIDTH.min(rect.width() * 0.5);
    let bands = if editor.gantt.minor_interval(rect.width() - header_width, MIN_TICK_SPACING) < GanttView::DAY {
        2.0
    } else {
        1.0
    };
    let ruler_height = (RULER_BAND * bands).min(rect.height());
    let header = egui::Rect::from_min_max(egui::pos2(rect.left(), rect.top() + ruler_height), egui::pos2(rect.left() + header_width, rect.bottom()));
    let ruler = egui::Rect::from_min_max(egui::pos2(rect.left() + header_width, rect.top()), egui::pos2(rect.right(), rect.top() + ruler_height));
    let body = egui::Rect::from_min_max(ruler.left_bottom(), rect.max);
    let corner = egui::Rect::from_min_max(rect.min, header.right_top());

    // Input is read only while the pointer is over this canvas, so the wheel
    // still scrolls whatever else is on screen and the keyboard is untouched.
    let response = ui.interact(rect, ui.id().with("gantt_canvas"), egui::Sense::click_and_drag());
    let rows_height = ROW_HEIGHT * plan.agents().len() as f32;
    let max_scroll = (rows_height - body.height()).max(0.0);
    if response.hovered() && body.width() > 0.0 {
        let (scroll, zoom, pointer) = ui.input(|input| (input.smooth_scroll_delta, f64::from(input.zoom_delta()), input.pointer.hover_pos()));
        if (zoom - 1.0).abs() > f64::EPSILON {
            // Anchored where the pointer is, so the instant under the cursor
            // stays under it - except where the window is already against the
            // time origin, which the view clamps for us.
            let anchor = pointer.map_or(0.5, |pos| f64::from((pos.x - body.left()) / body.width()));
            editor.gantt.zoom_at(zoom, anchor);
        }
        if scroll.x != 0.0 {
            editor.gantt.pan(-f64::from(scroll.x) / f64::from(body.width()) * editor.gantt.span_seconds);
        }
        if scroll.y != 0.0 {
            editor.gantt.row_scroll = (editor.gantt.row_scroll - scroll.y).clamp(0.0, max_scroll);
        }
    }
    if response.dragged_by(egui::PointerButton::Middle) && body.width() > 0.0 {
        editor.gantt.pan(-f64::from(response.drag_delta().x) / f64::from(body.width()) * editor.gantt.span_seconds);
    }
    // A row removed under a scrolled view must not leave the body blank.
    editor.gantt.row_scroll = editor.gantt.row_scroll.clamp(0.0, max_scroll);

    let visuals = ui.visuals().clone();
    let rule = visuals.widgets.noninteractive.bg_stroke;
    let (surface, stripe) = crate::ui::widgets::tree_row_colors(ui);
    ui.painter().rect_filled(rect, 0.0, surface);
    ui.painter().rect_filled(ruler, 0.0, visuals.widgets.noninteractive.bg_fill);
    ui.painter().rect_filled(corner, 0.0, visuals.widgets.noninteractive.bg_fill);

    let interval = editor.gantt.minor_interval(body.width(), MIN_TICK_SPACING);
    draw_ruler(ui, ruler, editor.gantt, interval, bands > 1.0);
    draw_rows(ui, header, body, stripe, editor, project);
    draw_grid(ui, body, editor.gantt, interval);

    // Rules last, so no row fill or grid line sits on top of them.
    let painter = ui.painter();
    painter.line_segment([ruler.left_bottom(), ruler.right_bottom()], rule);
    painter.line_segment([corner.right_top(), header.right_bottom()], rule);
    painter.rect_stroke(rect, 0.0, rule, egui::StrokeKind::Inside);

    if plan.agents().is_empty() {
        centred_note(ui, body, tr!("gantt-empty-fleet"));
    } else {
        // Below the rows rather than across them: said plainly, so lanes with
        // nothing in them are not mistaken for a schedule that came back
        // empty, but never painted over a row.
        let used = (rows_height - editor.gantt.row_scroll).max(0.0);
        let free = egui::Rect::from_min_max(egui::pos2(body.left(), body.top() + used), body.max);
        if free.height() > 32.0 {
            centred_note(ui, free, tr!("gantt-no-sequences"));
        }
    }
}

/// The time ruler: minor ticks with their labels, and - while the minor ticks
/// are finer than a day - a band of days above grouping them.
fn draw_ruler(ui: &egui::Ui, rect: egui::Rect, view: GanttView, interval: f64, day_band: bool) {
    if !rect.is_positive() {
        return;
    }
    let painter = ui.painter_at(rect);
    let rule = ui.visuals().widgets.noninteractive.bg_stroke;
    let text_color = ui.visuals().weak_text_color();
    let minor_band = egui::Rect::from_min_max(egui::pos2(rect.left(), rect.bottom() - RULER_BAND.min(rect.height())), rect.max);

    if day_band {
        // Whole days across the top. Drawn from the first day boundary at or
        // before the left edge so a part-visible day still carries its label.
        let first = (view.start_seconds / GanttView::DAY).floor();
        let mut day = first;
        while day * GanttView::DAY <= view.end_seconds() {
            let start = day * GanttView::DAY;
            let left = view.x_of(start, rect.left(), rect.width()).max(rect.left());
            let right = view.x_of(start + GanttView::DAY, rect.left(), rect.width()).min(rect.right());
            if right - left > 28.0 {
                painter.text(
                    egui::pos2(left + 6.0, rect.top() + RULER_BAND * 0.5),
                    egui::Align2::LEFT_CENTER,
                    tr!("gantt-day", day = day_of(start + 1.0).to_string()),
                    egui::TextStyle::Small.resolve(ui.style()),
                    text_color,
                );
            }
            if left > rect.left() {
                painter.line_segment([egui::pos2(left, rect.top()), egui::pos2(left, rect.bottom())], rule);
            }
            day += 1.0;
        }
        painter.line_segment([minor_band.left_top(), minor_band.right_top()], rule);
    }

    for tick in view.visible_ticks(interval) {
        let x = view.x_of(tick, rect.left(), rect.width());
        painter.line_segment([egui::pos2(x, minor_band.bottom() - 5.0), egui::pos2(x, minor_band.bottom())], rule);
        let label = if interval < GanttView::DAY {
            time_of(tick)
        } else {
            tr!("gantt-day", day = day_of(tick).to_string())
        };
        painter.text(
            egui::pos2(x + 4.0, minor_band.center().y),
            egui::Align2::LEFT_CENTER,
            label,
            egui::TextStyle::Small.resolve(ui.style()),
            text_color,
        );
    }
}

/// The agent column and the row bands beside it, drawn together so a row's
/// name and its lane cannot come apart.
fn draw_rows(ui: &mut egui::Ui, header: egui::Rect, body: egui::Rect, stripe: egui::Color32, editor: &EditorState, project: &UiProjectView) {
    let plan = &project.schedule;
    if !body.is_positive() {
        return;
    }
    let rule = ui.visuals().widgets.noninteractive.bg_stroke;
    let text_color = ui.visuals().text_color();
    let weak = ui.visuals().weak_text_color();
    let scroll = editor.gantt.row_scroll;
    // Only the rows the body can show are visited: several hundred agents
    // cost the same as a handful.
    let first = (scroll / ROW_HEIGHT).floor().max(0.0) as usize;
    let last = (((scroll + body.height()) / ROW_HEIGHT).ceil() as usize).min(plan.agents().len());

    for (index, agent) in plan.agents().iter().enumerate().take(last).skip(first) {
        let top = body.top() + ROW_HEIGHT * index as f32 - scroll;
        let row = egui::Rect::from_min_max(egui::pos2(body.left(), top), egui::pos2(body.right(), top + ROW_HEIGHT));
        let name_cell = egui::Rect::from_min_max(egui::pos2(header.left(), top), egui::pos2(header.right(), top + ROW_HEIGHT));
        if index % 2 == 1 {
            ui.painter_at(body).rect_filled(row, 0.0, stripe);
            ui.painter_at(header).rect_filled(name_cell, 0.0, stripe);
        }
        ui.painter_at(body).line_segment([row.left_bottom(), row.right_bottom()], rule);
        ui.painter_at(header).line_segment([name_cell.left_bottom(), name_cell.right_bottom()], rule);

        let class = plan.class(agent.class_id);
        let subtitle = match class {
            Some(class) => tr_format!(
                literal = "%class% · %rate% %unit%",
                class = class.name.clone(),
                rate = format!("{}", class.default_dig_rate_tph),
                unit = tr!("schedule-tph")
            ),
            None => tr!("schedule-error-unknown-class"),
        };
        // Truncated with the full text on hover, so a long machine name is
        // still readable without widening the column.
        let width = (name_cell.width() - 16.0).max(0.0);
        let name = egui::WidgetText::from(bold(&agent.name).color(text_color)).into_galley(ui, Some(egui::TextWrapMode::Truncate), width, egui::TextStyle::Body);
        let detail = egui::WidgetText::from(egui::RichText::new(&subtitle).small().color(weak)).into_galley(ui, Some(egui::TextWrapMode::Truncate), width, egui::TextStyle::Small);
        let painter = ui.painter_at(header);
        painter.galley(egui::pos2(name_cell.left() + 8.0, name_cell.top() + 4.0), name, text_color);
        painter.galley(egui::pos2(name_cell.left() + 8.0, name_cell.top() + 4.0 + ROW_HEIGHT * 0.45), detail, weak);
        let hover = name_cell.intersect(header);
        if hover.is_positive() {
            ui.interact(hover, ui.id().with(("gantt_row", agent.id)), egui::Sense::hover()).on_hover_text(tr_format!(
                literal = "%name%\n%detail%",
                name = agent.name.clone(),
                detail = subtitle
            ));
        }
    }
}

/// Vertical grid lines under the rows, on the same ticks the ruler labels.
fn draw_grid(ui: &egui::Ui, rect: egui::Rect, view: GanttView, interval: f64) {
    if !rect.is_positive() {
        return;
    }
    let painter = ui.painter_at(rect);
    let stroke = egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color.gamma_multiply(0.6));
    for tick in view.visible_ticks(interval) {
        let x = view.x_of(tick, rect.left(), rect.width());
        painter.line_segment([egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())], stroke);
    }
}

/// A single line of guidance across the middle of the timeline.
fn centred_note(ui: &egui::Ui, rect: egui::Rect, text: String) {
    ui.painter_at(rect).text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        egui::TextStyle::Body.resolve(ui.style()),
        ui.visuals().weak_text_color(),
    );
}
