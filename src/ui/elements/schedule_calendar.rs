//! Virtualized period calendar for loader availability, utilisation and rate.

use thousands::Separable;

use crate::{
    i18n::tr,
    model::schedule::{CalendarCell, CalendarCellEdit, CalendarField, CalendarPeriod, LoaderAgent, SCHEDULE_PERIOD_H, SchedulePlan},
    ui::{
        EditorState, UiProjectView, chrome,
        state::{CalendarCellAddress, CalendarCellDraft, CalendarSelection, PlanningSubpage, ScheduleEdit, ScheduleStep, UiCommand},
    },
};

const HIERARCHY_W: f32 = 240.0;
const DEFAULT_W: f32 = 100.0;
const PERIOD_W: f32 = 112.0;
const HEADER_H: f32 = 30.0;
const TOOLBAR_H: f32 = 34.0;
const OVERSCAN: i32 = 1;

#[derive(Clone, Copy)]
enum Row {
    Loaders,
    Agent(crate::model::schedule::LoaderAgentId),
    Field(crate::model::schedule::LoaderAgentId, CalendarField),
}

pub(crate) fn draw_details(ui: &mut egui::Ui, editor: &mut EditorState, project: &UiProjectView, commands: &mut Vec<UiCommand>) -> egui::Rect {
    if editor.schedule_calendar.runtime != project.active_session {
        editor.schedule_calendar = Default::default();
        editor.schedule_calendar.runtime = project.active_session;
    }
    let plan = &project.schedule;
    editor.schedule_calendar.visible_days = editor.schedule_calendar.visible_days.max(required_days(plan));
    egui::CentralPanel::default()
        .frame(chrome::region_frame(ui))
        .show(ui, |ui| {
            let rect = ui.available_rect_before_wrap();
            let toolbar = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), TOOLBAR_H.min(rect.height())));
            let grid = egui::Rect::from_min_max(egui::pos2(rect.left(), toolbar.bottom()), rect.max);
            draw_toolbar(ui, toolbar, editor);
            if plan.agents().is_empty() {
                draw_empty(ui, grid, editor);
            } else if grid.is_positive() {
                draw_grid(ui, grid, editor, plan, project.active_session, commands);
            }
            ui.allocate_rect(rect, egui::Sense::hover());
        })
        .response
        .rect
}

fn draw_toolbar(ui: &mut egui::Ui, rect: egui::Rect, editor: &mut EditorState) {
    let mut child = ui.new_child(egui::UiBuilder::new().id_salt("schedule_calendar_toolbar").max_rect(rect));
    child.set_clip_rect(child.clip_rect().intersect(rect));
    child.horizontal_centered(|ui| {
        if ui.button(tr!("schedule-calendar-extend")).clicked() {
            editor.schedule_calendar.visible_days = editor.schedule_calendar.visible_days.saturating_add(14);
        }
        ui.add_space(8.0);
        ui.label(tr!("schedule-calendar-jump"));
        let response = ui.add_sized([64.0, ui.spacing().interact_size.y], egui::TextEdit::singleline(&mut editor.schedule_calendar.jump_day));
        if response.lost_focus()
            && ui.input(|input| input.key_pressed(egui::Key::Enter))
            && let Ok(day) = editor.schedule_calendar.jump_day.trim().parse::<u32>()
            && day > 0
        {
            editor.schedule_calendar.visible_days = editor.schedule_calendar.visible_days.max(day);
            editor.schedule_calendar.scroll_x = day.saturating_sub(1) as f32 * PERIOD_W;
        }
        ui.menu_button("?", |ui| {
            ui.set_max_width(360.0);
            ui.label(tr!("schedule-calendar-help"));
        });
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

fn required_days(plan: &SchedulePlan) -> u32 {
    let override_day = plan
        .agents()
        .iter()
        .flat_map(|agent| agent.calendar.periods.keys())
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
    14_u32.max(override_day).max(bar_day)
}

fn rows(plan: &SchedulePlan, editor: &EditorState) -> Vec<Row> {
    let mut rows = vec![Row::Loaders];
    for agent in plan.agents() {
        rows.push(Row::Agent(agent.id));
        if !editor.schedule_calendar.collapsed.contains(&agent.id) {
            rows.extend([
                Row::Field(agent.id, CalendarField::Availability),
                Row::Field(agent.id, CalendarField::Utilisation),
                Row::Field(agent.id, CalendarField::Rate),
            ]);
        }
    }
    rows
}

fn draw_grid(ui: &mut egui::Ui, rect: egui::Rect, editor: &mut EditorState, plan: &SchedulePlan, session: u32, commands: &mut Vec<UiCommand>) {
    let row_h = crate::ui::widgets::explorer::row_height(ui);
    let body = egui::Rect::from_min_max(egui::pos2(rect.left(), rect.top() + HEADER_H), rect.max);
    let period_left = rect.left() + HIERARCHY_W + DEFAULT_W;
    let period_view = egui::Rect::from_min_max(egui::pos2(period_left, rect.top()), rect.max);
    let rows = rows(plan, editor);
    let max_y = (rows.len() as f32 * row_h - body.height()).max(0.0);
    let max_x = (editor.schedule_calendar.visible_days as f32 * PERIOD_W - period_view.width()).max(0.0);
    if ui.rect_contains_pointer(rect) {
        let scroll = ui.input(|input| input.smooth_scroll_delta);
        editor.schedule_calendar.scroll_y = (editor.schedule_calendar.scroll_y - scroll.y).clamp(0.0, max_y);
        editor.schedule_calendar.scroll_x = (editor.schedule_calendar.scroll_x - scroll.x).clamp(0.0, max_x);
    }
    editor.schedule_calendar.scroll_y = editor.schedule_calendar.scroll_y.clamp(0.0, max_y);
    editor.schedule_calendar.scroll_x = editor.schedule_calendar.scroll_x.clamp(0.0, max_x);

    handle_keyboard(ui, editor, plan, session, commands);
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
        draw_row(ui, row_rect, *row, editor, plan, first_period..last_period, period_left, session, commands);
    }
    ui.painter().rect_stroke(rect, 0.0, stroke, egui::StrokeKind::Inside);
    ui.painter()
        .line_segment([egui::pos2(hierarchy_head.right(), rect.top()), egui::pos2(hierarchy_head.right(), rect.bottom())], stroke);
    ui.painter()
        .line_segment([egui::pos2(default_head.right(), rect.top()), egui::pos2(default_head.right(), rect.bottom())], stroke);
    ui.painter()
        .line_segment([egui::pos2(rect.left(), body.top()), egui::pos2(rect.right(), body.top())], stroke);
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
    match row {
        Row::Loaders => {
            ui.put(
                hierarchy.shrink2(egui::vec2(8.0, 2.0)),
                egui::Label::new(crate::ui::fonts::bold(&tr!("schedule-calendar-loaders"))),
            );
        }
        Row::Agent(id) => {
            let Some(agent) = plan.agent(id) else { return };
            let collapsed = editor.schedule_calendar.collapsed.contains(&id);
            let label = format!("{}  {}", if collapsed { "▸" } else { "▾" }, agent.name);
            if ui.put(hierarchy.shrink2(egui::vec2(8.0, 2.0)), egui::Button::new(label).frame(false)).clicked() {
                if collapsed {
                    editor.schedule_calendar.collapsed.remove(&id);
                } else {
                    editor.schedule_calendar.collapsed.insert(id);
                }
            }
        }
        Row::Field(agent_id, field) => {
            let Some(agent) = plan.agent(agent_id) else { return };
            let label = match field {
                CalendarField::Availability => tr!("schedule-calendar-availability"),
                CalendarField::Utilisation => tr!("schedule-calendar-utilisation"),
                CalendarField::Rate => tr!("schedule-calendar-rate"),
            };
            ui.put(hierarchy.shrink2(egui::vec2(26.0, 2.0)), egui::Label::new(label).truncate().halign(egui::Align::Min));
            let default_rect = egui::Rect::from_min_max(egui::pos2(hierarchy.right(), rect.top()), egui::pos2(hierarchy.right() + DEFAULT_W, rect.bottom()));
            draw_cell(
                ui,
                default_rect,
                CalendarCellAddress {
                    agent: agent_id,
                    field,
                    cell: CalendarCell::Default,
                },
                editor,
                plan,
                agent,
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
                        agent: agent_id,
                        field,
                        cell: CalendarCell::Period(CalendarPeriod(period)),
                    },
                    editor,
                    plan,
                    agent,
                    session,
                    commands,
                );
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_cell(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    address: CalendarCellAddress,
    editor: &mut EditorState,
    plan: &SchedulePlan,
    agent: &LoaderAgent,
    session: u32,
    commands: &mut Vec<UiCommand>,
) {
    if !rect.is_positive() {
        return;
    }
    let selected = selected(editor, plan, address);
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
            move_selection(editor, plan, nav);
        }
    } else {
        let explicit = cell_text(plan, agent, address);
        let color = if selected { ui.visuals().selection.stroke.color } else { ui.visuals().text_color() };
        ui.painter().with_clip_rect(rect).text(
            rect.right_center() - egui::vec2(6.0, 0.0),
            egui::Align2::RIGHT_CENTER,
            explicit,
            egui::TextStyle::Body.resolve(ui.style()),
            color,
        );
    }
    response.on_hover_text(resolved_hover(plan, agent, address));
}

fn cell_id(ui: &egui::Ui, address: CalendarCellAddress) -> egui::Id {
    ui.id().with(("calendar_cell", address.agent, address.field, address.cell))
}

fn editable(address: CalendarCellAddress) -> bool {
    !(address.cell == CalendarCell::Default && address.field == CalendarField::Rate)
}

/// What this cell says in its own right, which is nothing at all for an
/// inherited one. The Default rate is read-only class data rather than an
/// authored value, so it has none either.
fn explicit_value(agent: &LoaderAgent, address: CalendarCellAddress) -> Option<f64> {
    match address.cell {
        CalendarCell::Default => match address.field {
            CalendarField::Availability => Some(agent.calendar.default_availability),
            CalendarField::Utilisation => Some(agent.calendar.default_utilisation),
            CalendarField::Rate => None,
        },
        CalendarCell::Period(period) => agent.calendar.periods.get(&period).and_then(|value| match address.field {
            CalendarField::Availability => value.availability,
            CalendarField::Utilisation => value.utilisation,
            CalendarField::Rate => value.rate_tph,
        }),
    }
}

fn explicit_text(agent: &LoaderAgent, address: CalendarCellAddress) -> String {
    explicit_value(agent, address).map(|value| format_value(address.field, value)).unwrap_or_default()
}

/// The same value as plain digits: what an editor opens on and what the
/// clipboard carries. The grid paints separators and units, and neither is
/// something [`parse_value`] is obliged to read back, so a cell the user
/// copies or re-opens must not hand them its painted form.
fn raw_text(agent: &LoaderAgent, address: CalendarCellAddress) -> String {
    explicit_value(agent, address).map(|value| trimmed_number(scaled(address.field, value))).unwrap_or_default()
}

fn cell_text(plan: &SchedulePlan, agent: &LoaderAgent, address: CalendarCellAddress) -> String {
    if address.cell == CalendarCell::Default && address.field == CalendarField::Rate {
        return plan
            .class(agent.class_id)
            .map(|class| format_value(CalendarField::Rate, class.default_dig_rate_tph))
            .unwrap_or_default();
    }
    explicit_text(agent, address)
}

fn resolved_hover(plan: &SchedulePlan, agent: &LoaderAgent, address: CalendarCellAddress) -> String {
    let Some(class) = plan.class(agent.class_id) else {
        return tr!("schedule-error-unknown-class");
    };
    if address.cell == CalendarCell::Default && address.field == CalendarField::Rate {
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
    let (value, source) = match address.field {
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
    tr!("schedule-calendar-resolved", value = format_value(address.field, value), source = source)
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
    let text = typed.unwrap_or_else(|| plan.agent(address.agent).map(|agent| raw_text(agent, address)).unwrap_or_default());
    editor.schedule_calendar.draft = Some(CalendarCellDraft {
        address,
        text,
        error: None,
        request_focus: true,
    });
}

fn commit_draft(editor: &mut EditorState, session: u32, commands: &mut Vec<UiCommand>) -> bool {
    let Some(draft) = editor.schedule_calendar.draft.as_mut() else { return true };
    match parse_value(draft.address.field, &draft.text) {
        Ok(value) => {
            commands.push(UiCommand::schedule(
                session,
                ScheduleEdit::SetCalendarCells {
                    edits: vec![CalendarCellEdit {
                        agent: draft.address.agent,
                        cell: draft.address.cell,
                        field: draft.address.field,
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

fn field_rows<'a>(plan: &'a SchedulePlan, editor: &'a EditorState) -> impl Iterator<Item = (crate::model::schedule::LoaderAgentId, CalendarField)> + 'a {
    plan.agents()
        .iter()
        .filter(|agent| !editor.schedule_calendar.collapsed.contains(&agent.id))
        .flat_map(|agent| {
            [CalendarField::Availability, CalendarField::Utilisation, CalendarField::Rate]
                .into_iter()
                .map(move |field| (agent.id, field))
        })
}

fn address_index(editor: &EditorState, plan: &SchedulePlan, address: CalendarCellAddress) -> Option<(usize, u32)> {
    let row = field_rows(plan, editor).position(|(agent, field)| agent == address.agent && field == address.field)?;
    let column = match address.cell {
        CalendarCell::Default => 0,
        CalendarCell::Period(period) => period.0.saturating_add(1),
    };
    Some((row, column))
}

fn address_at(editor: &EditorState, plan: &SchedulePlan, row: usize, column: u32) -> Option<CalendarCellAddress> {
    let (agent, field) = field_rows(plan, editor).nth(row)?;
    let cell = if column == 0 {
        CalendarCell::Default
    } else {
        CalendarCell::Period(CalendarPeriod(column - 1))
    };
    Some(CalendarCellAddress { agent, field, cell })
}

fn selection_bounds(editor: &EditorState, plan: &SchedulePlan) -> Option<(usize, usize, u32, u32)> {
    let selection = editor.schedule_calendar.selection?;
    let (ar, ac) = address_index(editor, plan, selection.anchor)?;
    let (fr, fc) = address_index(editor, plan, selection.focus)?;
    Some((ar.min(fr), ar.max(fr), ac.min(fc), ac.max(fc)))
}

fn selected(editor: &EditorState, plan: &SchedulePlan, address: CalendarCellAddress) -> bool {
    let Some((r0, r1, c0, c1)) = selection_bounds(editor, plan) else { return false };
    address_index(editor, plan, address).is_some_and(|(row, col)| (r0..=r1).contains(&row) && (c0..=c1).contains(&col))
}

fn move_selection(editor: &mut EditorState, plan: &SchedulePlan, nav: Nav) {
    let Some(selection) = editor.schedule_calendar.selection else { return };
    let Some((mut row, mut column)) = address_index(editor, plan, selection.focus) else {
        return;
    };
    let field_row_count = field_rows(plan, editor).count();
    if field_row_count == 0 {
        return;
    }
    loop {
        match nav {
            Nav::Left => column = column.saturating_sub(1),
            Nav::Right => column = column.saturating_add(1).min(editor.schedule_calendar.visible_days),
            Nav::Up => row = row.saturating_sub(1),
            Nav::Down => row = (row + 1).min(field_row_count - 1),
        }
        let Some(address) = address_at(editor, plan, row, column) else { return };
        if editable(address) {
            editor.schedule_calendar.selection = Some(CalendarSelection { anchor: address, focus: address });
            return;
        }
        if matches!(nav, Nav::Left) && column == 0 {
            return;
        }
    }
}

fn handle_keyboard(ui: &mut egui::Ui, editor: &mut EditorState, plan: &SchedulePlan, session: u32, commands: &mut Vec<UiCommand>) {
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
            egui::Event::Paste(text) => paste(editor, plan, session, commands, &text),
            egui::Event::Copy => copy(editor, plan, ui),
            egui::Event::Key {
                key, pressed: true, modifiers, ..
            } => match key {
                egui::Key::Enter => {
                    if let Some(selection) = editor.schedule_calendar.selection {
                        begin_edit(editor, plan, selection.focus, None);
                    }
                }
                egui::Key::ArrowLeft => move_selection(editor, plan, Nav::Left),
                egui::Key::ArrowRight => move_selection(editor, plan, Nav::Right),
                egui::Key::ArrowUp => move_selection(editor, plan, Nav::Up),
                egui::Key::ArrowDown => move_selection(editor, plan, Nav::Down),
                egui::Key::Backspace | egui::Key::Delete if !modifiers.command => clear_selection(editor, plan, session, commands),
                _ => {}
            },
            _ => {}
        }
    }
}

fn clear_selection(editor: &mut EditorState, plan: &SchedulePlan, session: u32, commands: &mut Vec<UiCommand>) {
    let Some((r0, r1, c0, c1)) = selection_bounds(editor, plan) else { return };
    let mut edits = Vec::new();
    for row in r0..=r1 {
        for column in c0..=c1 {
            let Some(address) = address_at(editor, plan, row, column).filter(|address| editable(*address)) else {
                continue;
            };
            edits.push(CalendarCellEdit {
                agent: address.agent,
                cell: address.cell,
                field: address.field,
                value: None,
            });
        }
    }
    if !edits.is_empty() {
        commands.push(UiCommand::schedule(session, ScheduleEdit::SetCalendarCells { edits }));
    }
}

fn paste(editor: &mut EditorState, plan: &SchedulePlan, session: u32, commands: &mut Vec<UiCommand>, text: &str) {
    let Some(selection) = editor.schedule_calendar.selection else { return };
    let Some((start_row, start_column)) = address_index(editor, plan, selection.focus) else {
        return;
    };
    let text = text.strip_suffix("\r\n").or_else(|| text.strip_suffix('\n')).unwrap_or(text);
    let rows: Vec<Vec<&str>> = text.lines().map(|line| line.trim_end_matches('\r').split('\t').collect()).collect();
    let mut edits = Vec::new();
    for (row_offset, values) in rows.iter().enumerate() {
        for (column_offset, text) in values.iter().enumerate() {
            let Some(address) = address_at(editor, plan, start_row + row_offset, start_column.saturating_add(column_offset as u32)) else {
                editor.schedule_calendar.error = Some(tr!("schedule-calendar-paste-outside"));
                return;
            };
            if !editable(address) {
                editor.schedule_calendar.error = Some(tr!("schedule-calendar-paste-read-only"));
                return;
            }
            let value = match parse_value(address.field, text) {
                Ok(value) => value,
                Err(error) => {
                    editor.schedule_calendar.error = Some(error);
                    return;
                }
            };
            edits.push(CalendarCellEdit {
                agent: address.agent,
                cell: address.cell,
                field: address.field,
                value,
            });
        }
    }
    editor.schedule_calendar.error = None;
    if !edits.is_empty() {
        commands.push(UiCommand::schedule(session, ScheduleEdit::SetCalendarCells { edits }));
    }
}

fn copy(editor: &EditorState, plan: &SchedulePlan, ui: &egui::Ui) {
    let Some((r0, r1, c0, c1)) = selection_bounds(editor, plan) else { return };
    let mut lines = Vec::new();
    for row in r0..=r1 {
        let mut cells = Vec::new();
        for column in c0..=c1 {
            let text = address_at(editor, plan, row, column)
                .and_then(|address| plan.agent(address.agent).map(|agent| raw_text(agent, address)))
                .unwrap_or_default();
            cells.push(text);
        }
        lines.push(cells.join("\t"));
    }
    ui.output_mut(|output| output.commands.push(egui::OutputCommand::CopyText(lines.join("\n"))));
}
