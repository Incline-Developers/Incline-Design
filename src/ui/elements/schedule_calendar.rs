//! Virtualized period calendar for loader availability, utilisation and rate.

use thousands::Separable;

use crate::{
    i18n::{tr, tr_format},
    model::{
        Document,
        schedule::{
            CalendarCell, CalendarCellEdit, CalendarField, CalendarPeriod, CrusherCell, CrusherCellEdit, CrusherOverride, DestinationId, DestinationKind, DestinationProduction,
            LoaderAgent, PeriodProduction, SCHEDULE_PERIOD_H, SchedulePlan, StandaloneDestinationId, destinations,
        },
    },
    ui::{
        EditorState, UiProjectView, chrome,
        state::{CalendarCellAddress, CalendarCellDraft, CalendarOwner, CalendarRow, CalendarSelection, PlanningSubpage, ScheduleEdit, ScheduleStep, UiCommand},
    },
};

const HIERARCHY_W: f32 = 240.0;
const DEFAULT_W: f32 = 100.0;
const PERIOD_W: f32 = 112.0;
const HEADER_H: f32 = 30.0;
const TOOLBAR_H: f32 = 34.0;
const OVERSCAN: i32 = 1;
/// Height of the day scroll bar along the bottom of the period columns.
const SCROLLBAR_H: f32 = 12.0;
/// How far each level of the row hierarchy is indented from the one above.
/// The gutter it leaves is shaded in the parent's own colour, so a loader's
/// rows read as sitting inside it rather than merely beside it.
const NEST_INDENT: f32 = 16.0;

/// One drawn row of the grid.
///
/// Two headings and two kinds of group, in one list: the virtualizer, the
/// selection and the clipboard all index the same list, so a destination row is
/// as much part of the rectangle as a loader row is.
#[derive(Clone, Copy)]
enum Row {
    Loaders,
    Destinations,
    Group(CalendarOwner),
    Field(CalendarOwner, CalendarRow),
}

/// What the Calendar needs to know about one destination: the two things Solids
/// and the rule list own between them, resolved once per frame.
#[derive(Clone)]
struct DestinationRow {
    id: DestinationId,
    name: String,
    kind: DestinationKind,
}

/// Every destination the Calendar shows, in the order the Setup pages list them.
///
/// Only present when routing is on: a project that does not route has nothing to
/// report per destination, and a group of empty rows would read as a result.
///
/// The plan comes from the *active project*, not from `document`: `document` is
/// the render-scene composite, which clones the reserve fields and the solids
/// every page reads but carries no schedule of its own.
fn destination_rows(plan: &SchedulePlan, document: &Document) -> Vec<DestinationRow> {
    let routing = plan.routing();
    if !routing.enabled {
        return Vec::new();
    }
    destinations::available(document.solids(), routing)
        .into_iter()
        .map(|entry| DestinationRow {
            id: entry.id,
            name: entry.name,
            kind: entry.kind,
        })
        .collect()
}

/// The rows one destination group shows, which depend on what it does with what
/// it receives.
fn destination_row_kinds(kind: DestinationKind) -> &'static [CalendarRow] {
    match kind {
        // A crusher has no storage, so it has no inventory to report - and it
        // has the one editable destination row, its daily budget.
        DestinationKind::Crusher => &[CalendarRow::CrusherLimit, CalendarRow::Received],
        DestinationKind::Stockpile | DestinationKind::Dump => &[CalendarRow::Received, CalendarRow::Cumulative],
    }
}

pub(crate) fn draw_details(ui: &mut egui::Ui, editor: &mut EditorState, project: &UiProjectView, document: &Document, commands: &mut Vec<UiCommand>) -> egui::Rect {
    if editor.schedule_calendar.runtime != project.active_session {
        editor.schedule_calendar = Default::default();
        editor.schedule_calendar.runtime = project.active_session;
    }
    let plan = &project.schedule;
    // Held by value for the frame: the grid needs the mirrored result while it
    // also holds the editor mutably, and the clone is one refcount.
    let production = editor.schedule_production.clone();
    let production = production.as_deref();
    let received = editor.schedule_received.clone();
    let received = received.as_deref();
    let destinations = destination_rows(plan, document);
    editor.schedule_calendar.visible_days = editor.schedule_calendar.visible_days.max(required_days(plan, production, received));
    egui::CentralPanel::default()
        .frame(chrome::region_frame(ui))
        .show(ui, |ui| {
            let rect = ui.available_rect_before_wrap();
            let toolbar = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), TOOLBAR_H.min(rect.height())));
            let grid = egui::Rect::from_min_max(egui::pos2(rect.left(), toolbar.bottom()), rect.max);
            draw_toolbar(ui, toolbar, editor, commands);
            if plan.agents().is_empty() {
                draw_empty(ui, grid, editor);
            } else if grid.is_positive() {
                draw_grid(ui, grid, editor, plan, &destinations, production, received, project.active_session, commands);
            }
            ui.allocate_rect(rect, egui::Sense::hover());
        })
        .response
        .rect
}

/// The Calendar's own toolbar: the schedule run controls, and whatever the
/// held result or the last edit has to say.
///
/// Days are reached by scrolling rather than by asking for them: the extent
/// already covers the authored overrides, the sequenced bars and the
/// calculated interval, so a button that added a fortnight of empty columns
/// and a box that jumped to one were two ways of saying the same thing the
/// scroll bar says.
fn draw_toolbar(ui: &mut egui::Ui, rect: egui::Rect, editor: &mut EditorState, commands: &mut Vec<UiCommand>) {
    let mut child = ui.new_child(egui::UiBuilder::new().id_salt("schedule_calendar_toolbar").max_rect(rect));
    child.set_clip_rect(child.clip_rect().intersect(rect));
    child.horizontal_centered(|ui| {
        super::schedule_gantt::draw_calculation_controls(ui, editor, "calendar", commands);
        if editor.schedule_run_stale {
            // Said once, here: the alternative is repeating it in every
            // calculated row of every loader.
            ui.add_space(8.0);
            ui.colored_label(ui.visuals().warn_fg_color, tr!("schedule-calendar-results-stale"));
        }
        if let Some(error) = &editor.schedule_calendar.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
    });
}

fn draw_empty(ui: &mut egui::Ui, rect: egui::Rect, editor: &mut EditorState) {
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        ui.centered_and_justified(|ui| {
            ui.vertical_centered(|ui| {
                ui.label(tr!("schedule-calendar-empty"));
                if ui.button(tr!("schedule-calendar-add-loader")).clicked() {
                    editor.schedule_subpage = PlanningSubpage::Setup;
                    editor.schedule_setup_step = ScheduleStep::LoaderAgents;
                }
            });
        });
    });
}

fn required_days(plan: &SchedulePlan, production: Option<&PeriodProduction>, received: Option<&DestinationProduction>) -> u32 {
    let override_day = plan
        .agents()
        .iter()
        .flat_map(|agent| agent.calendar.periods.keys())
        .chain(plan.routing().standalone.iter().flat_map(|entry| entry.crusher.periods.keys()))
        .map(|period| period.0.saturating_add(2))
        .max()
        .unwrap_or(0);
    let bar_day = plan
        .bars()
        .iter()
        .map(|bar| match bar.window.end_h {
            Some(end) => ((end / SCHEDULE_PERIOD_H).ceil() as u32).saturating_add(1),
            None => (bar.window.start_h / SCHEDULE_PERIOD_H).floor() as u32 + 2,
        })
        .max()
        .unwrap_or(0);
    // Whatever the calculation answers for has to be reachable, or its own
    // figures would sit past the end of the grid.
    let calculated_day = production
        .map_or(0, PeriodProduction::covered_periods)
        .max(received.map_or(0, DestinationProduction::covered_periods));
    14_u32.max(override_day).max(bar_day).max(calculated_day)
}

fn rows(plan: &SchedulePlan, destinations: &[DestinationRow], editor: &EditorState) -> Vec<Row> {
    let mut rows = vec![Row::Loaders];
    for agent in plan.agents() {
        let owner = CalendarOwner::Loader(agent.id);
        rows.push(Row::Group(owner));
        if !editor.schedule_calendar.collapsed.contains(&owner) {
            rows.extend(LOADER_ROWS.iter().map(|row| Row::Field(owner, *row)));
        }
    }
    if !destinations.is_empty() {
        rows.push(Row::Destinations);
        for destination in destinations {
            let owner = CalendarOwner::Destination(destination.id);
            rows.push(Row::Group(owner));
            if !editor.schedule_calendar.collapsed.contains(&owner) {
                rows.extend(destination_row_kinds(destination.kind).iter().map(|row| Row::Field(owner, *row)));
            }
        }
    }
    rows
}

/// The rows one loader group shows, in drawn order.
const LOADER_ROWS: [CalendarRow; 4] = [
    CalendarRow::Input(CalendarField::Availability),
    CalendarRow::Input(CalendarField::Utilisation),
    CalendarRow::Input(CalendarField::Rate),
    CalendarRow::Tonnes,
];

#[allow(clippy::too_many_arguments)]
fn draw_grid(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    editor: &mut EditorState,
    plan: &SchedulePlan,
    destinations: &[DestinationRow],
    production: Option<&PeriodProduction>,
    received: Option<&DestinationProduction>,
    session: u32,
    commands: &mut Vec<UiCommand>,
) {
    let row_h = crate::ui::widgets::explorer::row_height(ui);
    let scrollbar_top = (rect.bottom() - SCROLLBAR_H).max(rect.top() + HEADER_H);
    let body = egui::Rect::from_min_max(egui::pos2(rect.left(), rect.top() + HEADER_H), egui::pos2(rect.right(), scrollbar_top));
    let period_left = rect.left() + HIERARCHY_W + DEFAULT_W;
    let period_view = egui::Rect::from_min_max(egui::pos2(period_left, rect.top()), egui::pos2(rect.right(), scrollbar_top));
    let rows = rows(plan, destinations, editor);
    let max_y = (rows.len() as f32 * row_h - body.height()).max(0.0);
    let max_x = (editor.schedule_calendar.visible_days as f32 * PERIOD_W - period_view.width()).max(0.0);
    if ui.rect_contains_pointer(rect) {
        let scroll = ui.input(|input| input.smooth_scroll_delta);
        editor.schedule_calendar.scroll_y = (editor.schedule_calendar.scroll_y - scroll.y).clamp(0.0, max_y);
        editor.schedule_calendar.scroll_x = (editor.schedule_calendar.scroll_x - scroll.x).clamp(0.0, max_x);
    }
    editor.schedule_calendar.scroll_y = editor.schedule_calendar.scroll_y.clamp(0.0, max_y);
    editor.schedule_calendar.scroll_x = editor.schedule_calendar.scroll_x.clamp(0.0, max_x);

    handle_keyboard(ui, editor, plan, destinations, production, received, session, commands);
    let visuals = ui.visuals().clone();
    let stroke = visuals.widgets.noninteractive.bg_stroke;
    ui.painter().rect_filled(rect, 0.0, crate::ui::widgets::tree_row_colors(ui).1);
    let hierarchy_head = egui::Rect::from_min_size(rect.min, egui::vec2(HIERARCHY_W, HEADER_H));
    let default_head = egui::Rect::from_min_size(egui::pos2(hierarchy_head.right(), rect.top()), egui::vec2(DEFAULT_W, HEADER_H));
    paint_header(ui, hierarchy_head, tr!("schedule-calendar-setting"), None);
    paint_header(ui, default_head, tr!("schedule-calendar-default"), None);

    let first_period = ((editor.schedule_calendar.scroll_x / PERIOD_W).floor() as i32 - OVERSCAN).max(0) as u32;
    let last_period = (((editor.schedule_calendar.scroll_x + period_view.width()) / PERIOD_W).ceil() as i32 + OVERSCAN)
        .max(0)
        .min(editor.schedule_calendar.visible_days as i32) as u32;
    for period in first_period..last_period {
        let x = period_left + period as f32 * PERIOD_W - editor.schedule_calendar.scroll_x;
        let cell = egui::Rect::from_min_size(egui::pos2(x, rect.top()), egui::vec2(PERIOD_W, HEADER_H)).intersect(period_view);
        paint_header(
            ui,
            cell,
            tr!("schedule-calendar-day", day = period.saturating_add(1).to_string()),
            Some(tr!(
                "schedule-calendar-hours",
                start = format!("{:.0}", f64::from(period) * SCHEDULE_PERIOD_H),
                end = format!("{:.0}", f64::from(period.saturating_add(1)) * SCHEDULE_PERIOD_H)
            )),
        );
    }

    let first_row = ((editor.schedule_calendar.scroll_y / row_h).floor() as i32 - OVERSCAN).max(0) as usize;
    let last_row = (((editor.schedule_calendar.scroll_y + body.height()) / row_h).ceil() as i32 + OVERSCAN)
        .max(0)
        .min(rows.len() as i32) as usize;
    for (row_index, row) in rows.iter().enumerate().take(last_row).skip(first_row) {
        let y = body.top() + row_index as f32 * row_h - editor.schedule_calendar.scroll_y;
        let row_rect = egui::Rect::from_min_size(egui::pos2(rect.left(), y), egui::vec2(rect.width(), row_h)).intersect(body);
        draw_row(
            ui,
            row_rect,
            *row,
            editor,
            plan,
            destinations,
            production,
            received,
            first_period..last_period,
            period_left,
            session,
            commands,
        );
    }
    ui.painter().rect_stroke(rect, 0.0, stroke, egui::StrokeKind::Inside);
    ui.painter()
        .line_segment([egui::pos2(hierarchy_head.right(), rect.top()), egui::pos2(hierarchy_head.right(), body.bottom())], stroke);
    ui.painter()
        .line_segment([egui::pos2(default_head.right(), rect.top()), egui::pos2(default_head.right(), body.bottom())], stroke);
    ui.painter()
        .line_segment([egui::pos2(rect.left(), body.top()), egui::pos2(rect.right(), body.top())], stroke);
    draw_day_scrollbar(
        ui,
        egui::Rect::from_min_max(egui::pos2(period_left, scrollbar_top), rect.max),
        editor,
        period_view.width(),
        max_x,
    );
}

/// The day scroll bar: how the grid is moved across the horizon now that no
/// button extends it and no box jumps to a day.
///
/// Hand-painted like the grid above it, and for the same reason - the columns
/// are virtualized, so there is no laid-out content for a `ScrollArea` to
/// measure.
fn draw_day_scrollbar(ui: &mut egui::Ui, rect: egui::Rect, editor: &mut EditorState, view_width: f32, max_x: f32) {
    if !rect.is_positive() {
        return;
    }
    let total = max_x + view_width;
    ui.painter().rect_filled(rect, 0.0, ui.visuals().extreme_bg_color);
    if max_x <= 0.0 || total <= 0.0 || view_width <= 0.0 {
        return;
    }
    let track = rect.shrink2(egui::vec2(1.0, 3.0));
    let response = ui.interact(rect, ui.id().with("schedule_calendar_day_scroll"), egui::Sense::click_and_drag());
    // A thumb narrower than this cannot be grabbed, so a long horizon stops
    // shrinking it and gives up proportionality instead.
    const MIN_THUMB: f32 = 24.0;
    let thumb_w = (track.width() * view_width / total).clamp(MIN_THUMB.min(track.width()), track.width());
    let travel = track.width() - thumb_w;
    if let Some(pointer) = response.interact_pointer_pos()
        && travel > 0.0
    {
        // Dragged from wherever it was grabbed, so the thumb does not jump
        // its own half-width under the pointer on the first press.
        let grab = ui.id().with("schedule_calendar_day_scroll_grab");
        let offset = if response.drag_started() || response.clicked() {
            let thumb_x = track.left() + travel * (editor.schedule_calendar.scroll_x / max_x);
            let inside = pointer.x - thumb_x;
            let inside = if (0.0..=thumb_w).contains(&inside) { inside } else { thumb_w / 2.0 };
            ui.data_mut(|data| data.insert_temp(grab, inside));
            inside
        } else {
            ui.data(|data| data.get_temp::<f32>(grab)).unwrap_or(thumb_w / 2.0)
        };
        let fraction = ((pointer.x - offset - track.left()) / travel).clamp(0.0, 1.0);
        editor.schedule_calendar.scroll_x = fraction * max_x;
    }
    let thumb_x = track.left() + travel * (editor.schedule_calendar.scroll_x / max_x);
    let thumb = egui::Rect::from_min_size(egui::pos2(thumb_x, track.top()), egui::vec2(thumb_w, track.height()));
    let visuals = ui.style().interact(&response);
    ui.painter().rect_filled(thumb, track.height() / 2.0, visuals.bg_fill);
}

fn paint_header(ui: &mut egui::Ui, rect: egui::Rect, text: String, hover: Option<String>) {
    if !rect.is_positive() {
        return;
    }
    ui.painter().rect_filled(rect, 0.0, ui.visuals().widgets.noninteractive.bg_fill);
    let response = ui.put(rect.shrink(5.0), egui::Label::new(crate::ui::fonts::bold(&text)).truncate());
    if let Some(hover) = hover {
        response.on_hover_text(hover);
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_row(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    row: Row,
    editor: &mut EditorState,
    plan: &SchedulePlan,
    destinations: &[DestinationRow],
    production: Option<&PeriodProduction>,
    received: Option<&DestinationProduction>,
    periods: std::ops::Range<u32>,
    period_left: f32,
    session: u32,
    commands: &mut Vec<UiCommand>,
) {
    if !rect.is_positive() {
        return;
    }
    let stroke = ui.visuals().widgets.noninteractive.bg_stroke;
    ui.painter().line_segment([rect.left_bottom(), rect.right_bottom()], stroke);
    let hierarchy = egui::Rect::from_min_max(rect.min, egui::pos2(rect.left() + HIERARCHY_W, rect.bottom()));
    // The gutter every level above this row leaves to its left, each shaded
    // in that level's own colour. Unbroken down the column, so a loader's
    // three settings read as being inside the loader, which is inside Loaders.
    let depth = match row {
        Row::Loaders | Row::Destinations => 0,
        Row::Group(_) => 1,
        Row::Field(..) => 2,
    };
    for level in 0..depth {
        let band = egui::Rect::from_min_max(
            egui::pos2(hierarchy.left() + level as f32 * NEST_INDENT, rect.top()),
            egui::pos2(hierarchy.left() + (level + 1) as f32 * NEST_INDENT, rect.bottom()),
        );
        ui.painter().rect_filled(band, 0.0, nest_fill(ui, level));
    }
    let indent = depth as f32 * NEST_INDENT;
    let label_start = hierarchy.left() + indent;
    match row {
        Row::Loaders | Row::Destinations => {
            let label = if matches!(row, Row::Loaders) {
                tr!("schedule-calendar-loaders")
            } else {
                tr!("destination-destinations")
            };
            ui.painter().rect_filled(hierarchy, 0.0, nest_fill(ui, 0));
            paint_label(ui, hierarchy, label_start + 8.0, &label, LabelStyle::Heading);
        }
        Row::Group(owner) => {
            let name = match owner {
                CalendarOwner::Loader(id) => match plan.agent(id) {
                    Some(agent) => agent.name.clone(),
                    None => return,
                },
                CalendarOwner::Destination(id) => match destinations.iter().find(|entry| entry.id == id) {
                    Some(entry) => tr_format!(literal = "%name% · %kind%", name = entry.name.clone(), kind = entry.kind.label()),
                    None => return,
                },
            };
            let row_rect = egui::Rect::from_min_max(egui::pos2(label_start, rect.top()), egui::pos2(hierarchy.right(), rect.bottom()));
            ui.painter().rect_filled(row_rect, 0.0, nest_fill(ui, 1));
            let collapsed = editor.schedule_calendar.collapsed.contains(&owner);
            let label = format!("{}  {}", if collapsed { "▸" } else { "▾" }, name);
            // Interacted with rather than laid out as a button: a widget put
            // over the row would centre its text, and the indent is the whole
            // point of the row.
            let response = ui.interact(row_rect, ui.id().with(("schedule_calendar_group", owner)), egui::Sense::click());
            paint_label(ui, row_rect, label_start + 8.0, &label, LabelStyle::Heading);
            if response.clicked() {
                if collapsed {
                    editor.schedule_calendar.collapsed.remove(&owner);
                } else {
                    editor.schedule_calendar.collapsed.insert(owner);
                }
            }
        }
        Row::Field(owner, kind) => {
            let agent = match owner {
                CalendarOwner::Loader(id) => match plan.agent(id) {
                    Some(agent) => Some(agent),
                    None => return,
                },
                CalendarOwner::Destination(id) => {
                    if !destinations.iter().any(|entry| entry.id == id) {
                        return;
                    }
                    None
                }
            };
            let destination_kind = match owner {
                CalendarOwner::Destination(id) => destinations.iter().find(|entry| entry.id == id).map(|entry| entry.kind),
                CalendarOwner::Loader(_) => None,
            };
            let text_left = label_start + 18.0;
            let label = row_label(kind, destination_kind);
            if kind.is_calculated() {
                // A lock beside a quieter label: the tint alone would be the
                // only thing saying this row cannot be typed into, and colour
                // alone is not enough to say it.
                paint_lock(ui, egui::pos2(text_left - 10.0, rect.center().y), ui.visuals().weak_text_color());
                paint_label(ui, hierarchy, text_left, &label, LabelStyle::Calculated);
            } else {
                paint_label(ui, hierarchy, text_left, &label, LabelStyle::Field);
            }
            let default_rect = egui::Rect::from_min_max(egui::pos2(hierarchy.right(), rect.top()), egui::pos2(hierarchy.right() + DEFAULT_W, rect.bottom()));
            draw_cell(
                ui,
                default_rect,
                CalendarCellAddress {
                    owner,
                    row: kind,
                    cell: CalendarCell::Default,
                },
                editor,
                plan,
                destinations,
                agent,
                production,
                received,
                session,
                commands,
            );
            for period in periods {
                let x = period_left + period as f32 * PERIOD_W - editor.schedule_calendar.scroll_x;
                let cell = egui::Rect::from_min_size(egui::pos2(x, rect.top()), egui::vec2(PERIOD_W, rect.height()))
                    .intersect(egui::Rect::from_min_max(egui::pos2(period_left, rect.top()), rect.max));
                draw_cell(
                    ui,
                    cell,
                    CalendarCellAddress {
                        owner,
                        row: kind,
                        cell: CalendarCell::Period(CalendarPeriod(period)),
                    },
                    editor,
                    plan,
                    destinations,
                    agent,
                    production,
                    received,
                    session,
                    commands,
                );
            }
        }
    }
}

/// What one row is called. A crusher's receipts are what it *processed*, since
/// nothing is buffered in this increment; a stockpile's cumulative figure is
/// scheduled inventory rather than stock on hand, because nothing is reclaimed.
fn row_label(row: CalendarRow, destination: Option<DestinationKind>) -> String {
    match row {
        CalendarRow::Input(CalendarField::Availability) => tr!("schedule-calendar-availability"),
        CalendarRow::Input(CalendarField::Utilisation) => tr!("schedule-calendar-utilisation"),
        CalendarRow::Input(CalendarField::Rate) => tr!("schedule-calendar-rate"),
        CalendarRow::Tonnes => tr!("schedule-calendar-tonnes"),
        CalendarRow::CrusherLimit => tr!("destination-calendar-limit"),
        CalendarRow::Received => match destination {
            Some(DestinationKind::Crusher) => tr!("destination-calendar-processed"),
            _ => tr!("destination-calendar-received"),
        },
        CalendarRow::Cumulative => match destination {
            Some(DestinationKind::Dump) => tr!("destination-calendar-deposited"),
            _ => tr!("destination-calendar-inventory"),
        },
    }
}

/// How a row label is drawn: the two hierarchy levels are headings, a
/// setting's name is ordinary text, and a calculated row's is quieter.
#[derive(Clone, Copy, PartialEq, Eq)]
enum LabelStyle {
    Heading,
    Field,
    Calculated,
}

/// One row label, left-aligned at its own indent and clipped to the hierarchy
/// column.
///
/// Painted rather than laid out: a widget put into the row would centre its
/// text, and centred text says nothing about which level it sits at.
fn paint_label(ui: &egui::Ui, row: egui::Rect, left: f32, text: &str, style: LabelStyle) {
    let size = egui::TextStyle::Body.resolve(ui.style()).size;
    let (font, color) = match style {
        LabelStyle::Heading => (crate::ui::fonts::bold_font(size), ui.visuals().strong_text_color()),
        LabelStyle::Field => (egui::FontId::proportional(size), ui.visuals().text_color()),
        LabelStyle::Calculated => (egui::FontId::proportional(size), ui.visuals().weak_text_color()),
    };
    let available = (row.right() - 8.0 - left).max(0.0);
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_owned(), font, color);
    job.wrap.max_width = available;
    job.wrap.max_rows = 1;
    let galley = ui.painter().layout_job(job);
    ui.painter()
        .with_clip_rect(row)
        .galley(egui::pos2(left, row.center().y - galley.size().y / 2.0), galley, color);
}

/// The shade one level of the row hierarchy is drawn in.
///
/// Level 0 is the header fill the column titles use, and each level below it
/// steps back towards the row background, so the gutters read as nested bands
/// without a second palette to keep in step with the theme.
fn nest_fill(ui: &egui::Ui, level: usize) -> egui::Color32 {
    let header = ui.visuals().widgets.noninteractive.bg_fill;
    let background = crate::ui::widgets::tree_row_colors(ui).1;
    match level {
        0 => header,
        _ => blend(header, background, 0.5),
    }
}

/// Mix two colours, so a nesting band can be derived from the theme's own
/// fills rather than from constants that only suit one theme.
fn blend(from: egui::Color32, to: egui::Color32, t: f32) -> egui::Color32 {
    let mix = |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * t).round().clamp(0.0, 255.0) as u8;
    egui::Color32::from_rgb(mix(from.r(), to.r()), mix(from.g(), to.g()), mix(from.b(), to.b()))
}

/// A padlock a few pixels across: the shackle first, then the body over its
/// lower half, so the two shapes read as one silhouette at this size.
fn paint_lock(ui: &egui::Ui, center: egui::Pos2, color: egui::Color32) {
    let body = egui::Rect::from_center_size(center + egui::vec2(0.0, 2.0), egui::vec2(7.0, 5.0));
    ui.painter().circle_stroke(egui::pos2(center.x, body.top()), 2.2, egui::Stroke::new(1.2, color));
    ui.painter().rect_filled(body, 1.0, color);
}

#[allow(clippy::too_many_arguments)]
fn draw_cell(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    address: CalendarCellAddress,
    editor: &mut EditorState,
    plan: &SchedulePlan,
    destinations: &[DestinationRow],
    agent: Option<&LoaderAgent>,
    production: Option<&PeriodProduction>,
    received: Option<&DestinationProduction>,
    session: u32,
    commands: &mut Vec<UiCommand>,
) {
    if !rect.is_positive() {
        return;
    }
    let selected = selected(editor, plan, destinations, address);
    if address.row.is_calculated() {
        ui.painter().rect_filled(rect, 0.0, ui.visuals().faint_bg_color);
    }
    if selected {
        ui.painter().rect_filled(rect, 0.0, ui.visuals().selection.bg_fill);
    }
    let response = ui.interact(rect, cell_id(ui, address), egui::Sense::click());
    if response.clicked() {
        if !commit_draft(editor, session, commands) {
            return;
        }
        let extend = ui.input(|input| input.modifiers.shift);
        editor.schedule_calendar.selection = Some(match (extend, editor.schedule_calendar.selection) {
            (true, Some(selection)) => CalendarSelection { focus: address, ..selection },
            _ => CalendarSelection { anchor: address, focus: address },
        });
        editor.schedule_calendar.error = None;
    }
    if response.double_clicked() && editable(address) {
        begin_edit(editor, plan, address, None);
    }
    if editor.schedule_calendar.draft.as_ref().is_some_and(|draft| draft.address == address) {
        let mut commit = false;
        let mut abandon = false;
        let mut move_key = None;
        {
            let draft = editor.schedule_calendar.draft.as_mut().expect("checked above");
            let edit = ui.put(rect.shrink2(egui::vec2(3.0, 2.0)), egui::TextEdit::singleline(&mut draft.text));
            if draft.request_focus {
                edit.request_focus();
                draft.request_focus = false;
            }
            // Enter, Tab and Escape each make the editor surrender focus, so
            // the keys arrive on the frame it reports `lost_focus` and never
            // while it still holds focus.
            if edit.has_focus() || edit.lost_focus() {
                let (escape, enter, tab, shift) = ui.input(|input| {
                    (
                        input.key_pressed(egui::Key::Escape),
                        input.key_pressed(egui::Key::Enter),
                        input.key_pressed(egui::Key::Tab),
                        input.modifiers.shift,
                    )
                });
                if escape {
                    abandon = true;
                } else if enter {
                    commit = true;
                    move_key = Some(Nav::Down);
                } else if tab {
                    commit = true;
                    move_key = Some(if shift { Nav::Left } else { Nav::Right });
                } else if edit.lost_focus() {
                    // Focus went somewhere else - another cell, the toolbar -
                    // which commits the draft rather than stranding an editor
                    // nobody is typing in.
                    commit = true;
                }
            }
        }
        if abandon || commit {
            // Tab hands focus to the next widget, and the grid reads the
            // keyboard only while nothing else holds it: without this the cell
            // the caret just moved to would ignore what the user types next.
            ui.memory_mut(|memory| memory.stop_text_input());
        }
        if abandon {
            editor.schedule_calendar.draft = None;
            editor.schedule_calendar.error = None;
        } else if commit
            && commit_draft(editor, session, commands)
            && let Some(nav) = move_key
        {
            move_selection(editor, plan, destinations, nav);
        }
    } else {
        let color = if selected { ui.visuals().selection.stroke.color } else { ui.visuals().text_color() };
        ui.painter().with_clip_rect(rect).text(
            rect.right_center() - egui::vec2(6.0, 0.0),
            egui::Align2::RIGHT_CENTER,
            display_text(plan, agent, production, received, address),
            egui::TextStyle::Body.resolve(ui.style()),
            color,
        );
    }
    if let Some(hover) = hover_text(plan, agent, production, received, address) {
        response.on_hover_text(hover);
    }
}

/// What a cell shows. A calculated cell says nothing at all outside the
/// calculated interval, and marks a period the interval stops part-way through.
fn display_text(
    plan: &SchedulePlan,
    agent: Option<&LoaderAgent>,
    production: Option<&PeriodProduction>,
    received: Option<&DestinationProduction>,
    address: CalendarCellAddress,
) -> String {
    let marked = |figure: Option<(f64, bool)>| match figure {
        Some((tonnes, true)) => format!("{} *", format_tonnes(tonnes)),
        Some((tonnes, false)) => format_tonnes(tonnes),
        None => String::new(),
    };
    match address.row {
        CalendarRow::Input(_) => agent.map(|agent| cell_text(plan, agent, address)).unwrap_or_default(),
        CalendarRow::Tonnes => marked(calculated(production, address)),
        CalendarRow::CrusherLimit => crusher_text(plan, address),
        CalendarRow::Received => marked(destination_figure(received, address, false)),
        CalendarRow::Cumulative => marked(destination_figure(received, address, true)),
    }
}

fn hover_text(
    plan: &SchedulePlan,
    agent: Option<&LoaderAgent>,
    production: Option<&PeriodProduction>,
    received: Option<&DestinationProduction>,
    address: CalendarCellAddress,
) -> Option<String> {
    match address.row {
        CalendarRow::Input(_) => agent.map(|agent| resolved_hover(plan, agent, address)),
        CalendarRow::Tonnes => {
            let production = production?;
            calculated(Some(production), address)?
                .1
                .then(|| tr!("schedule-calendar-tonnes-partial", hours = trimmed_number(production.coverage_end_h())))
        }
        CalendarRow::CrusherLimit => Some(crusher_hover(plan, address)),
        CalendarRow::Received | CalendarRow::Cumulative => {
            let received = received?;
            let cumulative = address.row == CalendarRow::Cumulative;
            destination_figure(Some(received), address, cumulative)?
                .1
                .then(|| tr!("schedule-calendar-tonnes-partial", hours = trimmed_number(received.coverage_end_h())))
        }
    }
}

/// One destination row's figure and whether its period is only partly covered.
fn destination_figure(received: Option<&DestinationProduction>, address: CalendarCellAddress, cumulative: bool) -> Option<(f64, bool)> {
    let CalendarCell::Period(period) = address.cell else { return None };
    let received = received?;
    let destination = address.destination()?;
    let tonnes = if cumulative {
        received.cumulative(destination, period)?
    } else {
        received.received(destination, period)?
    };
    Some((tonnes, received.is_partial(period)))
}

/// A crusher cell's own text.
///
/// Blank means *inherit* on a period and *unlimited* on the default, which is
/// why an explicit unlimited period is spelled out: inheritance and "no limit
/// today, whatever the default says" have to stay distinguishable.
fn crusher_text(plan: &SchedulePlan, address: CalendarCellAddress) -> String {
    let Some(calendar) = crusher_calendar(plan, address) else { return String::new() };
    match address.cell {
        CalendarCell::Default => match calendar.default_tpd {
            Some(limit) => format_tonnes(limit),
            None => tr!("destination-unlimited"),
        },
        CalendarCell::Period(period) => match calendar.periods.get(&period) {
            None => String::new(),
            Some(CrusherOverride::Unlimited) => tr!("destination-unlimited"),
            Some(CrusherOverride::Limit(limit)) => format_tonnes(*limit),
        },
    }
}

fn crusher_hover(plan: &SchedulePlan, address: CalendarCellAddress) -> String {
    let Some(calendar) = crusher_calendar(plan, address) else {
        return tr!("destination-error-not-a-crusher");
    };
    let period = match address.cell {
        CalendarCell::Default => return tr!("destination-calendar-default-hover"),
        CalendarCell::Period(period) => period,
    };
    let resolved = match calendar.limit_at(period) {
        Some(limit) => tr_format!(literal = "%value% t/day", value = format_tonnes(limit)),
        None => tr!("destination-unlimited"),
    };
    let source = if calendar.periods.contains_key(&period) {
        tr!("schedule-calendar-explicit")
    } else {
        tr!("destination-calendar-inherited")
    };
    tr!("schedule-calendar-resolved", value = resolved, source = source)
}

/// The crusher calendar this cell edits, when the row belongs to one.
fn crusher_calendar(plan: &SchedulePlan, address: CalendarCellAddress) -> Option<&crate::model::schedule::CrusherCalendar> {
    let DestinationId::Standalone(id) = address.destination()? else { return None };
    plan.routing().crusher(id)
}

/// The crusher a crusher-limit edit is addressed to.
fn crusher_target(address: CalendarCellAddress) -> Option<StandaloneDestinationId> {
    match address.destination()? {
        DestinationId::Standalone(id) => Some(id),
        DestinationId::Solid(_) => None,
    }
}

/// This cell's calculated tonnes and whether its period is only partly
/// covered, or `None` where the calculation answers for nothing: no current
/// result, a period beyond the calculated interval, or the Default column,
/// which no calculation has an opinion about.
fn calculated(production: Option<&PeriodProduction>, address: CalendarCellAddress) -> Option<(f64, bool)> {
    let CalendarCell::Period(period) = address.cell else { return None };
    let production = production?;
    Some((production.tonnes(address.agent()?, period)?, production.is_partial(period)))
}

fn cell_id(ui: &egui::Ui, address: CalendarCellAddress) -> egui::Id {
    ui.id().with(("calendar_cell", address.owner, address.row, address.cell))
}

fn editable(address: CalendarCellAddress) -> bool {
    match address.row {
        // Calculated: selectable so it can be copied, and nothing more.
        CalendarRow::Tonnes | CalendarRow::Received | CalendarRow::Cumulative => false,
        CalendarRow::CrusherLimit => crusher_target(address).is_some(),
        CalendarRow::Input(field) => !(address.cell == CalendarCell::Default && field == CalendarField::Rate),
    }
}

/// What this cell says in its own right, which is nothing at all for an
/// inherited one. The Default rate is read-only class data rather than an
/// authored value, so it has none either.
fn explicit_value(agent: &LoaderAgent, address: CalendarCellAddress) -> Option<f64> {
    let field = address.row.field()?;
    match address.cell {
        CalendarCell::Default => match field {
            CalendarField::Availability => Some(agent.calendar.default_availability),
            CalendarField::Utilisation => Some(agent.calendar.default_utilisation),
            CalendarField::Rate => None,
        },
        CalendarCell::Period(period) => agent.calendar.periods.get(&period).and_then(|value| match field {
            CalendarField::Availability => value.availability,
            CalendarField::Utilisation => value.utilisation,
            CalendarField::Rate => value.rate_tph,
        }),
    }
}

fn explicit_text(agent: &LoaderAgent, address: CalendarCellAddress) -> String {
    match (address.row.field(), explicit_value(agent, address)) {
        (Some(field), Some(value)) => format_value(field, value),
        _ => String::new(),
    }
}

/// The same value as plain digits: what an editor opens on and what the
/// clipboard carries. The grid paints separators and units, and neither is
/// something [`parse_value`] is obliged to read back, so a cell the user
/// copies or re-opens must not hand them its painted form.
fn raw_text(agent: &LoaderAgent, address: CalendarCellAddress) -> String {
    match (address.row.field(), explicit_value(agent, address)) {
        (Some(field), Some(value)) => trimmed_number(scaled(field, value)),
        _ => String::new(),
    }
}

/// Plain digits for any cell: what an editor opens on and what the clipboard
/// carries, whichever half of the grid the cell is in.
fn raw_cell_text(plan: &SchedulePlan, address: CalendarCellAddress) -> String {
    match address.row {
        CalendarRow::CrusherLimit => match (crusher_calendar(plan, address), address.cell) {
            (Some(calendar), CalendarCell::Default) => calendar.default_tpd.map(trimmed_number).unwrap_or_else(|| tr!("destination-unlimited")),
            (Some(calendar), CalendarCell::Period(period)) => match calendar.periods.get(&period) {
                None => String::new(),
                Some(CrusherOverride::Unlimited) => tr!("destination-unlimited"),
                Some(CrusherOverride::Limit(limit)) => trimmed_number(*limit),
            },
            (None, _) => String::new(),
        },
        _ => address.agent().and_then(|id| plan.agent(id)).map(|agent| raw_text(agent, address)).unwrap_or_default(),
    }
}

/// Which crusher-calendar cell an address names.
fn crusher_cell(cell: CalendarCell) -> CrusherCell {
    match cell {
        CalendarCell::Default => CrusherCell::Default,
        CalendarCell::Period(period) => CrusherCell::Period(period),
    }
}

/// Parse a typed crusher budget.
///
/// Three answers, because a crusher cell has three states. An empty period cell
/// inherits the default; the unlimited word - or an infinity sign - says this
/// period has no limit whatever the default is; anything else is a figure. On the
/// default cell there is nothing to inherit, so empty and unlimited coincide -
/// which is exactly why an explicit unlimited has to exist for the periods.
fn parse_crusher(text: &str) -> Result<Option<CrusherOverride>, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let unlimited = tr!("destination-unlimited");
    if trimmed.eq_ignore_ascii_case(unlimited.trim()) || trimmed == "∞" {
        return Ok(Some(CrusherOverride::Unlimited));
    }
    // Separators are painted, so a figure copied out of the grid and pasted back
    // has to read as the number it was.
    let number = trimmed.replace(',', "");
    let parsed = number.parse::<f64>().map_err(|_| tr!("schedule-calendar-invalid-number"))?;
    if !parsed.is_finite() || parsed < 0.0 {
        return Err(crate::model::schedule::ScheduleError::InvalidCapacity.message());
    }
    Ok(Some(CrusherOverride::Limit(parsed)))
}

fn cell_text(plan: &SchedulePlan, agent: &LoaderAgent, address: CalendarCellAddress) -> String {
    if address.cell == CalendarCell::Default && address.row == CalendarRow::Input(CalendarField::Rate) {
        return plan
            .class(agent.class_id)
            .map(|class| format_value(CalendarField::Rate, class.default_dig_rate_tph))
            .unwrap_or_default();
    }
    explicit_text(agent, address)
}

fn resolved_hover(plan: &SchedulePlan, agent: &LoaderAgent, address: CalendarCellAddress) -> String {
    let Some(field) = address.row.field() else { return String::new() };
    let Some(class) = plan.class(agent.class_id) else {
        return tr!("schedule-error-unknown-class");
    };
    if address.cell == CalendarCell::Default && field == CalendarField::Rate {
        return tr!(
            "schedule-calendar-class-default",
            value = format_value(CalendarField::Rate, class.default_dig_rate_tph),
            class = class.name.clone()
        );
    }
    let period = match address.cell {
        CalendarCell::Default => CalendarPeriod(0),
        CalendarCell::Period(period) => period,
    };
    let Ok(values) = agent.calendar.values_at(period, class.default_dig_rate_tph) else {
        return tr!("schedule-calendar-invalid");
    };
    let (value, source) = match field {
        CalendarField::Availability => (
            values.availability,
            if explicit_value(agent, address).is_none() {
                tr!("schedule-calendar-loader-default")
            } else {
                tr!("schedule-calendar-explicit")
            },
        ),
        CalendarField::Utilisation => (
            values.utilisation,
            if explicit_value(agent, address).is_none() {
                tr!("schedule-calendar-loader-default")
            } else {
                tr!("schedule-calendar-explicit")
            },
        ),
        CalendarField::Rate => (
            values.rate_tph,
            if explicit_value(agent, address).is_none() {
                tr!("schedule-calendar-class-source")
            } else {
                tr!("schedule-calendar-explicit")
            },
        ),
    };
    tr!("schedule-calendar-resolved", value = format_value(field, value), source = source)
}

/// Percentages are stored as fractions and shown out of a hundred.
fn scaled(field: CalendarField, value: f64) -> f64 {
    if field == CalendarField::Rate { value } else { value * 100.0 }
}

fn trimmed_number(value: f64) -> String {
    let mut text = format!("{value:.3}");
    while text.contains('.') && text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    text
}

/// Tonnes to at most one decimal. Presentation only: the aggregation keeps
/// full precision, and this rounding never feeds back into it.
fn tonnes_number(value: f64) -> String {
    let rounded = (value * 10.0).round() / 10.0;
    if rounded.fract() == 0.0 { format!("{rounded:.0}") } else { format!("{rounded:.1}") }
}

pub(crate) fn format_tonnes(value: f64) -> String {
    tonnes_number(value).separate_with_commas()
}

fn format_value(field: CalendarField, value: f64) -> String {
    let text = trimmed_number(scaled(field, value)).separate_with_commas();
    match field {
        CalendarField::Rate => format!("{text} t/h"),
        _ => format!("{text}%"),
    }
}

fn parse_value(field: CalendarField, text: &str) -> Result<Option<f64>, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let number = match field {
        CalendarField::Availability | CalendarField::Utilisation => trimmed.strip_suffix('%').unwrap_or(trimmed).trim(),
        CalendarField::Rate => trimmed,
    };
    let parsed = number.parse::<f64>().map_err(|_| tr!("schedule-calendar-invalid-number"))?;
    match field {
        CalendarField::Availability | CalendarField::Utilisation if parsed.is_finite() && (0.0..=100.0).contains(&parsed) => Ok(Some(parsed / 100.0)),
        CalendarField::Rate if parsed.is_finite() && parsed > 0.0 => Ok(Some(parsed)),
        CalendarField::Availability | CalendarField::Utilisation => Err(tr!("schedule-calendar-invalid-percentage")),
        CalendarField::Rate => Err(tr!("schedule-error-invalid-rate")),
    }
}

fn begin_edit(editor: &mut EditorState, plan: &SchedulePlan, address: CalendarCellAddress, typed: Option<String>) {
    if !editable(address) {
        return;
    }
    let text = typed.unwrap_or_else(|| raw_cell_text(plan, address));
    editor.schedule_calendar.draft = Some(CalendarCellDraft {
        address,
        text,
        error: None,
        request_focus: true,
    });
}

fn commit_draft(editor: &mut EditorState, session: u32, commands: &mut Vec<UiCommand>) -> bool {
    let Some(draft) = editor.schedule_calendar.draft.as_mut() else { return true };
    // A crusher budget is the one destination input, and it commits through its
    // own edit: the loader calendar and the crusher calendar are different
    // things with different cells.
    if draft.address.row == CalendarRow::CrusherLimit {
        let Some(destination) = crusher_target(draft.address) else {
            editor.schedule_calendar.draft = None;
            return true;
        };
        match parse_crusher(&draft.text) {
            Ok(value) => {
                commands.push(UiCommand::schedule(
                    session,
                    ScheduleEdit::SetCrusherCells {
                        edits: vec![CrusherCellEdit {
                            destination,
                            cell: crusher_cell(draft.address.cell),
                            value,
                        }],
                    },
                ));
                editor.schedule_calendar.draft = None;
                editor.schedule_calendar.error = None;
                return true;
            }
            Err(error) => {
                draft.error = Some(error.clone());
                draft.request_focus = true;
                editor.schedule_calendar.error = Some(error);
                return false;
            }
        }
    }
    // A draft only ever opens on an editable cell, so this field is always
    // there; a calculated row has none and can reach no further than here.
    let (Some(field), Some(agent)) = (draft.address.row.field(), draft.address.agent()) else {
        editor.schedule_calendar.draft = None;
        return true;
    };
    match parse_value(field, &draft.text) {
        Ok(value) => {
            commands.push(UiCommand::schedule(
                session,
                ScheduleEdit::SetCalendarCells {
                    edits: vec![CalendarCellEdit {
                        agent,
                        cell: draft.address.cell,
                        field,
                        value,
                    }],
                },
            ));
            editor.schedule_calendar.draft = None;
            editor.schedule_calendar.error = None;
            true
        }
        Err(error) => {
            draft.error = Some(error.clone());
            // Keep the caret in the cell the user has to correct: the key that
            // asked to commit has already surrendered focus, and a draft
            // nothing can type into would swallow the next Escape too.
            draft.request_focus = true;
            editor.schedule_calendar.error = Some(error);
            false
        }
    }
}

#[derive(Clone, Copy)]
enum Nav {
    Left,
    Right,
    Up,
    Down,
}

/// Every row a loader group shows, in drawn order - the calculated row
/// included. Row numbers are what a pasted rectangle is measured against, so
/// leaving it out here would land pasted values on the wrong loader.
fn grid_rows(plan: &SchedulePlan, destinations: &[DestinationRow], editor: &EditorState) -> Vec<(CalendarOwner, CalendarRow)> {
    let mut rows = Vec::new();
    for agent in plan.agents() {
        let owner = CalendarOwner::Loader(agent.id);
        if editor.schedule_calendar.collapsed.contains(&owner) {
            continue;
        }
        rows.extend(LOADER_ROWS.iter().map(|row| (owner, *row)));
    }
    for destination in destinations {
        let owner = CalendarOwner::Destination(destination.id);
        if editor.schedule_calendar.collapsed.contains(&owner) {
            continue;
        }
        rows.extend(destination_row_kinds(destination.kind).iter().map(|row| (owner, *row)));
    }
    rows
}

fn address_index(editor: &EditorState, plan: &SchedulePlan, destinations: &[DestinationRow], address: CalendarCellAddress) -> Option<(usize, u32)> {
    let row = grid_rows(plan, destinations, editor)
        .into_iter()
        .position(|(owner, row)| owner == address.owner && row == address.row)?;
    let column = match address.cell {
        CalendarCell::Default => 0,
        CalendarCell::Period(period) => period.0.saturating_add(1),
    };
    Some((row, column))
}

fn address_at(editor: &EditorState, plan: &SchedulePlan, destinations: &[DestinationRow], row: usize, column: u32) -> Option<CalendarCellAddress> {
    let (owner, kind) = grid_rows(plan, destinations, editor).into_iter().nth(row)?;
    let cell = if column == 0 {
        CalendarCell::Default
    } else {
        CalendarCell::Period(CalendarPeriod(column - 1))
    };
    Some(CalendarCellAddress { owner, row: kind, cell })
}

fn selection_bounds(editor: &EditorState, plan: &SchedulePlan, destinations: &[DestinationRow]) -> Option<(usize, usize, u32, u32)> {
    let selection = editor.schedule_calendar.selection?;
    let (ar, ac) = address_index(editor, plan, destinations, selection.anchor)?;
    let (fr, fc) = address_index(editor, plan, destinations, selection.focus)?;
    Some((ar.min(fr), ar.max(fr), ac.min(fc), ac.max(fc)))
}

fn selected(editor: &EditorState, plan: &SchedulePlan, destinations: &[DestinationRow], address: CalendarCellAddress) -> bool {
    let Some((r0, r1, c0, c1)) = selection_bounds(editor, plan, destinations) else {
        return false;
    };
    address_index(editor, plan, destinations, address).is_some_and(|(row, col)| (r0..=r1).contains(&row) && (c0..=c1).contains(&col))
}

fn move_selection(editor: &mut EditorState, plan: &SchedulePlan, destinations: &[DestinationRow], nav: Nav) {
    let Some(selection) = editor.schedule_calendar.selection else { return };
    let Some((mut row, mut column)) = address_index(editor, plan, destinations, selection.focus) else {
        return;
    };
    let row_count = grid_rows(plan, destinations, editor).len();
    if row_count == 0 {
        return;
    }
    loop {
        let from = (row, column);
        match nav {
            Nav::Left => column = column.saturating_sub(1),
            Nav::Right => column = column.saturating_add(1).min(editor.schedule_calendar.visible_days),
            Nav::Up => row = row.saturating_sub(1),
            Nav::Down => row = (row + 1).min(row_count - 1),
        }
        // Clamped against the edge of the grid. Without this the search for the
        // next editable cell would never end when the last row is a calculated
        // one, which it always is.
        if (row, column) == from {
            return;
        }
        let Some(address) = address_at(editor, plan, destinations, row, column) else { return };
        if editable(address) {
            editor.schedule_calendar.selection = Some(CalendarSelection { anchor: address, focus: address });
            return;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_keyboard(
    ui: &mut egui::Ui,
    editor: &mut EditorState,
    plan: &SchedulePlan,
    destinations: &[DestinationRow],
    production: Option<&PeriodProduction>,
    received: Option<&DestinationProduction>,
    session: u32,
    commands: &mut Vec<UiCommand>,
) {
    // Cells are ordinary focusable widgets, so a Tab that moves along the row
    // also lands egui's focus on the cell the selection moved to. That focus is
    // this grid's own; only focus somewhere else - the jump field, a button -
    // means the keys belong to something other than the selection.
    let selected_cell = editor.schedule_calendar.selection.map(|selection| cell_id(ui, selection.focus));
    let focused = ui.memory(|memory| memory.focused());
    if editor.schedule_calendar.draft.is_some() || (focused.is_some() && focused != selected_cell) {
        return;
    }
    let events = ui.input(|input| input.events.clone());
    for event in events {
        match event {
            egui::Event::Text(text) if !text.chars().all(char::is_whitespace) => {
                if let Some(selection) = editor.schedule_calendar.selection {
                    begin_edit(editor, plan, selection.focus, Some(text));
                }
            }
            egui::Event::Paste(text) => paste(editor, plan, destinations, session, commands, &text),
            egui::Event::Copy => copy(editor, plan, destinations, production, received, ui),
            egui::Event::Key {
                key, pressed: true, modifiers, ..
            } => match key {
                egui::Key::Enter => {
                    if let Some(selection) = editor.schedule_calendar.selection {
                        begin_edit(editor, plan, selection.focus, None);
                    }
                }
                egui::Key::ArrowLeft => move_selection(editor, plan, destinations, Nav::Left),
                egui::Key::ArrowRight => move_selection(editor, plan, destinations, Nav::Right),
                egui::Key::ArrowUp => move_selection(editor, plan, destinations, Nav::Up),
                egui::Key::ArrowDown => move_selection(editor, plan, destinations, Nav::Down),
                egui::Key::Backspace | egui::Key::Delete if !modifiers.command => clear_selection(editor, plan, destinations, session, commands),
                _ => {}
            },
            _ => {}
        }
    }
}

fn clear_selection(editor: &mut EditorState, plan: &SchedulePlan, destinations: &[DestinationRow], session: u32, commands: &mut Vec<UiCommand>) {
    let Some((r0, r1, c0, c1)) = selection_bounds(editor, plan, destinations) else { return };
    let mut edits = Vec::new();
    let mut crusher_edits = Vec::new();
    for row in r0..=r1 {
        for column in c0..=c1 {
            let Some(address) = address_at(editor, plan, destinations, row, column) else { continue };
            if address.row.is_calculated() {
                editor.schedule_calendar.error = Some(tr!("schedule-calendar-calculated-selection"));
                return;
            }
            if address.row == CalendarRow::CrusherLimit {
                if let Some(destination) = crusher_target(address) {
                    crusher_edits.push(CrusherCellEdit {
                        destination,
                        cell: crusher_cell(address.cell),
                        value: None,
                    });
                }
                continue;
            }
            let (Some(field), Some(agent)) = (address.row.field().filter(|_| editable(address)), address.agent()) else {
                continue;
            };
            edits.push(CalendarCellEdit {
                agent,
                cell: address.cell,
                field,
                value: None,
            });
        }
    }
    editor.schedule_calendar.error = None;
    if !edits.is_empty() {
        commands.push(UiCommand::schedule(session, ScheduleEdit::SetCalendarCells { edits }));
    }
    if !crusher_edits.is_empty() {
        commands.push(UiCommand::schedule(session, ScheduleEdit::SetCrusherCells { edits: crusher_edits }));
    }
}

fn paste(editor: &mut EditorState, plan: &SchedulePlan, destinations: &[DestinationRow], session: u32, commands: &mut Vec<UiCommand>, text: &str) {
    let Some(selection) = editor.schedule_calendar.selection else { return };
    let Some((start_row, start_column)) = address_index(editor, plan, destinations, selection.focus) else {
        return;
    };
    let text = text.strip_suffix("\r\n").or_else(|| text.strip_suffix('\n')).unwrap_or(text);
    let rows: Vec<Vec<&str>> = text.lines().map(|line| line.trim_end_matches('\r').split('\t').collect()).collect();
    let mut edits = Vec::new();
    let mut crusher_edits = Vec::new();
    for (row_offset, values) in rows.iter().enumerate() {
        for (column_offset, text) in values.iter().enumerate() {
            let Some(address) = address_at(editor, plan, destinations, start_row + row_offset, start_column.saturating_add(column_offset as u32)) else {
                editor.schedule_calendar.error = Some(tr!("schedule-calendar-paste-outside"));
                return;
            };
            if address.row.is_calculated() {
                editor.schedule_calendar.error = Some(tr!("schedule-calendar-calculated-selection"));
                return;
            }
            if address.row == CalendarRow::CrusherLimit {
                let Some(destination) = crusher_target(address) else {
                    editor.schedule_calendar.error = Some(tr!("schedule-calendar-paste-read-only"));
                    return;
                };
                match parse_crusher(text) {
                    Ok(value) => crusher_edits.push(CrusherCellEdit {
                        destination,
                        cell: crusher_cell(address.cell),
                        value,
                    }),
                    Err(error) => {
                        editor.schedule_calendar.error = Some(error);
                        return;
                    }
                }
                continue;
            }
            let (Some(field), Some(agent)) = (address.row.field().filter(|_| editable(address)), address.agent()) else {
                editor.schedule_calendar.error = Some(tr!("schedule-calendar-paste-read-only"));
                return;
            };
            let value = match parse_value(field, text) {
                Ok(value) => value,
                Err(error) => {
                    editor.schedule_calendar.error = Some(error);
                    return;
                }
            };
            edits.push(CalendarCellEdit {
                agent,
                cell: address.cell,
                field,
                value,
            });
        }
    }
    editor.schedule_calendar.error = None;
    if !edits.is_empty() {
        commands.push(UiCommand::schedule(session, ScheduleEdit::SetCalendarCells { edits }));
    }
    if !crusher_edits.is_empty() {
        commands.push(UiCommand::schedule(session, ScheduleEdit::SetCrusherCells { edits: crusher_edits }));
    }
}

fn copy(
    editor: &EditorState,
    plan: &SchedulePlan,
    destinations: &[DestinationRow],
    production: Option<&PeriodProduction>,
    received: Option<&DestinationProduction>,
    ui: &egui::Ui,
) {
    let Some((r0, r1, c0, c1)) = selection_bounds(editor, plan, destinations) else { return };
    let mut lines = Vec::new();
    for row in r0..=r1 {
        let mut cells = Vec::new();
        for column in c0..=c1 {
            // Plain digits throughout, calculated cells included: separators
            // and the partial-period mark are things the grid paints, not
            // things a clipboard should carry.
            let text = match address_at(editor, plan, destinations, row, column) {
                Some(address) => match address.row {
                    CalendarRow::Tonnes => calculated(production, address).map(|(tonnes, _)| tonnes_number(tonnes)).unwrap_or_default(),
                    CalendarRow::Received => destination_figure(received, address, false).map(|(tonnes, _)| tonnes_number(tonnes)).unwrap_or_default(),
                    CalendarRow::Cumulative => destination_figure(received, address, true).map(|(tonnes, _)| tonnes_number(tonnes)).unwrap_or_default(),
                    CalendarRow::CrusherLimit | CalendarRow::Input(_) => raw_cell_text(plan, address),
                },
                None => String::new(),
            };
            cells.push(text);
        }
        lines.push(cells.join("\t"));
    }
    ui.output_mut(|output| output.commands.push(egui::OutputCommand::CopyText(lines.join("\n"))));
}
