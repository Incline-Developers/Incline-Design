//! The Delays step of Schedule Setup: delay types, delay lists and rosters.
//!
//! The index on the left lists the three; the editor beside it opens the one
//! selected. A delay list is a table - machine, start, end - typed in or
//! pasted from a spreadsheet, and every edit to it replaces the whole table so
//! each is one undo step. See [`crate::model::schedule::delays`] for what each
//! kind of delay means to the schedule.

use crate::{
    i18n::tr,
    model::schedule::{
        DelayEntry, DelayTypeId, LoaderAgentId, LoaderSelection, Roster, SchedulePlan,
        delays::{DelayRowProblem, MIN_ROSTER_DURATION_H, MIN_ROSTER_EVERY_H, hours_text, instant_parts, parse_delay_rows, parse_instant},
    },
    ui::{
        EditorState,
        state::{DelayDraft, DelaySelection, ScheduleEdit, UiCommand},
        widgets::{
            context_menu::{ContextMenuAction, context_menu_popup},
            data_grid::{DataGrid, GridRow, grid_named_row, grid_row, grid_separator_row},
            explorer::{explorer_note, row_height},
            toolbar::GROUP_CORNER_RADIUS,
        },
    },
};

/// An instant the way the Gantt writes it, which is also a way to type one.
pub(crate) fn instant_text(hours: f64) -> String {
    let (day, time) = instant_parts(hours);
    tr!("gantt-day-time", day = day.to_string(), time = time)
}

pub(crate) fn delay_color(color: [u8; 3]) -> egui::Color32 {
    egui::Color32::from_rgb(color[0], color[1], color[2])
}

/// The colour delays with no type are drawn in.
pub(crate) const UNTYPED_DELAY_COLOR: egui::Color32 = egui::Color32::from_rgb(0x8A, 0x8F, 0x98);

/// The colour and name a delay of `kind` is shown with.
pub(crate) fn delay_look(plan: &SchedulePlan, kind: Option<DelayTypeId>) -> (egui::Color32, String) {
    match kind.and_then(|kind| plan.delays().delay_type(kind)) {
        Some(entry) => (delay_color(entry.color), entry.name.clone()),
        None => (UNTYPED_DELAY_COLOR, tr!("delay-untyped")),
    }
}

fn type_label(plan: &SchedulePlan, kind: Option<DelayTypeId>) -> String {
    delay_look(plan, kind).1
}

fn type_options(plan: &SchedulePlan) -> Vec<(Option<DelayTypeId>, String)> {
    std::iter::once((None, tr!("delay-untyped")))
        .chain(plan.delays().types.iter().map(|entry| (Some(entry.id), entry.name.clone())))
        .collect()
}

fn agent_name(plan: &SchedulePlan, agent: LoaderAgentId) -> String {
    plan.agent(agent).map_or_else(|| tr!("schedule-error-unknown-agent"), |agent| agent.name.clone())
}

/// The left column: Delay Types, then each delay list, then each roster.
pub(crate) fn draw_delay_index(ui: &mut egui::Ui, rect: egui::Rect, editor: &mut EditorState, plan: &SchedulePlan, session: u32, commands: &mut Vec<UiCommand>) {
    let delays = plan.delays();
    let mut selected = editor.schedule_selected_delay;
    if selected.is_none() {
        selected = Some(DelaySelection::Types);
    }
    let new_list = |commands: &mut Vec<UiCommand>| {
        let name = crate::model::schedule::suggested_name(&tr!("delay-list-default"), delays.lists.iter().map(|list| list.title.clone()));
        commands.push(UiCommand::schedule(session, ScheduleEdit::AddDelayList { title: name }));
    };
    let new_roster = |commands: &mut Vec<UiCommand>| {
        let name = crate::model::schedule::suggested_name(&tr!("delay-roster-default"), delays.rosters.iter().map(|roster| roster.name.clone()));
        commands.push(UiCommand::schedule(session, ScheduleEdit::AddRoster { name }));
    };
    DataGrid::new("schedule_delay_index", rect, &tr!("delay-step")).show(ui, |ui| {
        let label = tr!("delay-types-row", count = delays.types.len().to_string());
        if grid_row(ui, GridRow::new(&label).selected(selected == Some(DelaySelection::Types))).clicked() {
            selected = Some(DelaySelection::Types);
        }

        grid_separator_row(ui, &tr!("delay-lists"), 0);
        if delays.lists.is_empty() {
            explorer_note(ui, tr!("delay-no-lists"));
        }
        for list in &delays.lists {
            let label = tr!("delay-list-row", title = list.title.clone(), count = list.entries.len().to_string());
            let response = grid_row(ui, GridRow::new(&label).selected(selected == Some(DelaySelection::List(list.id))));
            if response.clicked() {
                selected = Some(DelaySelection::List(list.id));
            }
            context_menu_popup(&response, &list.title, |ui| {
                if ContextMenuAction::new(tr!("delay-delete-list")).show(ui).clicked() {
                    commands.push(UiCommand::schedule(session, ScheduleEdit::DeleteDelayList(list.id)));
                    ui.close();
                }
            });
        }

        grid_separator_row(ui, &tr!("delay-rosters"), 0);
        if delays.rosters.is_empty() {
            explorer_note(ui, tr!("delay-no-rosters"));
        }
        for roster in &delays.rosters {
            let response = grid_row(ui, GridRow::new(&roster.name).selected(selected == Some(DelaySelection::Roster(roster.id))));
            if response.clicked() {
                selected = Some(DelaySelection::Roster(roster.id));
            }
            context_menu_popup(&response, &roster.name, |ui| {
                if ContextMenuAction::new(tr!("delay-delete-roster")).show(ui).clicked() {
                    commands.push(UiCommand::schedule(session, ScheduleEdit::DeleteRoster(roster.id)));
                    ui.close();
                }
            });
        }

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.add_space(6.0);
            if ui.add(egui::Button::new(tr!("delay-new-list")).corner_radius(GROUP_CORNER_RADIUS)).clicked() {
                new_list(commands);
            }
            if ui.add(egui::Button::new(tr!("delay-new-roster")).corner_radius(GROUP_CORNER_RADIUS)).clicked() {
                new_roster(commands);
            }
        });

        let body = ui.available_rect_before_wrap();
        if body.is_positive() {
            let response = ui.interact(body, ui.id().with("new_delay_space"), egui::Sense::click());
            context_menu_popup(&response, tr!("delay-step"), |ui| {
                if ContextMenuAction::new(tr!("delay-new-list")).show(ui).clicked() {
                    new_list(commands);
                    ui.close();
                }
                if ContextMenuAction::new(tr!("delay-new-roster")).show(ui).clicked() {
                    new_roster(commands);
                    ui.close();
                }
            });
        }
    });
    // A selection whose list or roster has gone falls back to the types.
    selected = match selected {
        Some(DelaySelection::List(id)) if delays.list(id).is_none() => Some(DelaySelection::Types),
        Some(DelaySelection::Roster(id)) if delays.roster(id).is_none() => Some(DelaySelection::Types),
        other => other,
    };
    editor.schedule_selected_delay = selected;
}

/// The selected delay type table, delay list or roster.
pub(crate) fn draw_delay_editor(ui: &mut egui::Ui, rect: egui::Rect, editor: &mut EditorState, plan: &SchedulePlan, session: u32, commands: &mut Vec<UiCommand>) {
    match editor.schedule_selected_delay.unwrap_or(DelaySelection::Types) {
        DelaySelection::Types => draw_types(ui, rect, editor, plan, session, commands),
        DelaySelection::List(id) => draw_list(ui, rect, editor, plan, id, session, commands),
        DelaySelection::Roster(id) => draw_roster(ui, rect, editor, plan, id, session, commands),
    }
}

/// The draft for `selection`, reopened when what it was opened from changed.
fn draft_for(editor: &mut EditorState, selection: DelaySelection, source: String, open: impl FnOnce() -> DelayDraft) -> &mut DelayDraft {
    if editor
        .schedule_delay_draft
        .as_ref()
        .is_none_or(|draft| draft.selection != Some(selection) || draft.source != source)
    {
        let mut draft = open();
        draft.selection = Some(selection);
        draft.source = source;
        editor.schedule_delay_draft = Some(draft);
    }
    editor.schedule_delay_draft.as_mut().expect("just ensured")
}

/// One row of cells across the width of the grid: the widths are fractions
/// of what is left once the fixed `trailing` width is taken off the right.
fn table_row(ui: &mut egui::Ui, fractions: &[f32], trailing: f32) -> (egui::Response, Vec<egui::Rect>, egui::Rect) {
    let height = row_height(ui) + 3.0;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), height), egui::Sense::click());
    let visuals = ui.visuals();
    let fill = if response.hovered() {
        visuals.widgets.hovered.bg_fill
    } else {
        crate::ui::widgets::tree_row_colors(ui).1
    };
    ui.painter().rect_filled(rect, 0.0, fill);
    let stroke = visuals.widgets.noninteractive.bg_stroke;
    ui.painter().line_segment([rect.left_bottom(), rect.right_bottom()], stroke);
    let inner = rect.shrink2(egui::vec2(6.0, 2.0));
    let width = (inner.width() - trailing).max(0.0);
    let mut cells = Vec::with_capacity(fractions.len());
    let mut left = inner.left();
    for fraction in fractions {
        let right = left + width * fraction;
        cells.push(egui::Rect::from_min_max(egui::pos2(left, inner.top()), egui::pos2(right - 4.0, inner.bottom())));
        left = right;
    }
    let end = egui::Rect::from_min_max(egui::pos2(inner.right() - trailing, inner.top()), inner.right_bottom());
    (response, cells, end)
}

fn header_row(ui: &mut egui::Ui, fractions: &[f32], trailing: f32, labels: &[String]) {
    let height = row_height(ui) + 3.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), height), egui::Sense::hover());
    let visuals = ui.visuals();
    ui.painter().rect_filled(rect, 0.0, visuals.widgets.noninteractive.bg_fill);
    ui.painter()
        .line_segment([rect.left_bottom(), rect.right_bottom()], visuals.widgets.noninteractive.bg_stroke);
    let inner = rect.shrink2(egui::vec2(6.0, 2.0));
    let width = (inner.width() - trailing).max(0.0);
    let mut left = inner.left();
    for (fraction, label) in fractions.iter().zip(labels) {
        let cell = egui::Rect::from_min_max(egui::pos2(left, inner.top()), egui::pos2(left + width * fraction, inner.bottom()));
        ui.painter_at(cell).text(
            egui::pos2(cell.left() + 4.0, cell.center().y),
            egui::Align2::LEFT_CENTER,
            label,
            egui::TextStyle::Body.resolve(ui.style()),
            ui.visuals().strong_text_color(),
        );
        left += width * fraction;
    }
}

fn combo_in<T: Clone + PartialEq>(ui: &mut egui::Ui, id: egui::Id, cell: egui::Rect, value: &mut T, selected_text: String, options: Vec<(T, String)>) -> bool {
    let mut changed = false;
    ui.scope_builder(egui::UiBuilder::new().id_salt(id).max_rect(cell), |ui| {
        ui.set_clip_rect(ui.clip_rect().intersect(cell));
        ui.spacing_mut().interact_size.y = cell.height();
        egui::ComboBox::from_id_salt(id.with("combo"))
            .selected_text(selected_text)
            .width(cell.width())
            .truncate()
            .show_ui(ui, |ui| {
                for (option, text) in options {
                    changed |= ui.selectable_value(value, option, text).changed();
                }
            });
    });
    changed
}

fn delete_button(ui: &mut egui::Ui, cell: egui::Rect, hint: String) -> bool {
    let cell = egui::Rect::from_center_size(cell.center() - egui::vec2(4.0, 0.0), egui::vec2(20.0, cell.height()));
    ui.put(cell, egui::Button::new("×").frame(false)).on_hover_text(hint).clicked()
}

/// Every delay type: its colour and its name, with a row to add another.
fn draw_types(ui: &mut egui::Ui, rect: egui::Rect, editor: &mut EditorState, plan: &SchedulePlan, session: u32, commands: &mut Vec<UiCommand>) {
    let types = &plan.delays().types;
    let source = format!("{types:?}");
    let draft = draft_for(editor, DelaySelection::Types, source, || DelayDraft {
        type_names: types.iter().map(|entry| entry.name.clone()).collect(),
        ..DelayDraft::default()
    });
    let mut edits = Vec::new();
    let fractions = [0.15, 0.85];
    DataGrid::new("schedule_delay_types", rect, &tr!("delay-types")).show(ui, |ui| {
        explorer_note(ui, tr!("delay-types-note"));
        header_row(ui, &fractions, 32.0, &[tr!("delay-colour"), tr!("planning-name")]);
        for (index, entry) in types.iter().enumerate() {
            let (_, cells, end) = table_row(ui, &fractions, 32.0);
            let mut color = delay_color(entry.color);
            let response = ui
                .scope_builder(egui::UiBuilder::new().id_salt(("delay_type_color", entry.id.0)).max_rect(cells[0]), |ui| {
                    ui.spacing_mut().interact_size.y = cells[0].height();
                    egui::color_picker::color_edit_button_srgba(ui, &mut color, egui::color_picker::Alpha::Opaque)
                })
                .inner;
            if response.changed() {
                edits.push(UiCommand::schedule(
                    session,
                    ScheduleEdit::SetDelayTypeColor {
                        kind: entry.id,
                        color: [color.r(), color.g(), color.b()],
                    },
                ));
            }
            let Some(name) = draft.type_names.get_mut(index) else { continue };
            let taken = types.iter().filter(|other| other.id != entry.id).map(|other| other.name.clone());
            let problem = super::schedule_setup::name_problem(name, taken.clone());
            let response = ui.put(cells[1], egui::TextEdit::singleline(name).desired_width(cells[1].width()));
            if let Some(message) = &problem {
                response.clone().on_hover_text(message);
            }
            if response.lost_focus() && problem.is_none() && name.trim() != entry.name {
                edits.push(UiCommand::schedule(
                    session,
                    ScheduleEdit::RenameDelayType {
                        kind: entry.id,
                        name: name.trim().to_owned(),
                    },
                ));
            }
            if delete_button(ui, end, tr!("delay-delete-type")) {
                edits.push(UiCommand::schedule(session, ScheduleEdit::DeleteDelayType(entry.id)));
            }
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.add_space(6.0);
            if ui.add(egui::Button::new(tr!("delay-new-type")).corner_radius(GROUP_CORNER_RADIUS)).clicked() {
                let name = crate::model::schedule::suggested_name(&tr!("delay-type-default"), types.iter().map(|entry| entry.name.clone()));
                edits.push(UiCommand::schedule(
                    session,
                    ScheduleEdit::AddDelayType {
                        name,
                        color: plan.delays().suggested_color(),
                    },
                ));
            }
        });
    });
    commands.append(&mut edits);
}

fn problem_text(line: usize, problem: &DelayRowProblem) -> String {
    let what = match problem {
        DelayRowProblem::Columns => tr!("delay-paste-columns"),
        DelayRowProblem::UnknownMachine(name) => tr!("delay-paste-machine", name = name.clone()),
        DelayRowProblem::Start(text) => tr!("delay-paste-start", text = text.clone()),
        DelayRowProblem::End(text) => tr!("delay-paste-end", text = text.clone()),
        DelayRowProblem::Order => tr!("delay-paste-order"),
    };
    tr!("delay-paste-line", line = line.to_string(), problem = what)
}

/// One delay list: its title and type, then its table.
#[allow(clippy::too_many_lines, reason = "one table, read top to bottom")]
fn draw_list(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    editor: &mut EditorState,
    plan: &SchedulePlan,
    id: crate::model::schedule::DelayListId,
    session: u32,
    commands: &mut Vec<UiCommand>,
) {
    let Some(list) = plan.delays().list(id) else { return };
    let source = format!("{list:?}");
    let draft = draft_for(editor, DelaySelection::List(id), source, || DelayDraft {
        title: list.title.clone(),
        cells: list.entries.iter().map(|entry| [instant_text(entry.start_h), instant_text(entry.end_h)]).collect(),
        ..DelayDraft::default()
    });
    let agents: Vec<(LoaderAgentId, String)> = plan.agents().iter().map(|agent| (agent.id, agent.name.clone())).collect();
    let mut edits = Vec::new();
    let mut entries = list.entries.clone();
    let mut entries_changed = false;
    let mut paste_error: Option<String> = None;

    // A spreadsheet's rows pasted while the pointer is over the table, with no
    // cell being typed in, are appended to it.
    let typing = ui.ctx().egui_wants_keyboard_input();
    let hovered = ui.rect_contains_pointer(rect);
    if hovered && !typing && draft.paste.is_none() {
        let pasted = ui.input(|input| {
            input.events.iter().find_map(|event| match event {
                egui::Event::Paste(text) => Some(text.clone()),
                _ => None,
            })
        });
        if let Some(text) = pasted {
            match parse_delay_rows(&text, &agents) {
                Ok(rows) => {
                    entries.extend(rows);
                    entries_changed = true;
                }
                Err(problems) => paste_error = Some(problems.iter().map(|(line, problem)| problem_text(*line, problem)).collect::<Vec<_>>().join("\n")),
            }
        }
    }

    let fractions = [0.34, 0.27, 0.27, 0.12];
    DataGrid::new("schedule_delay_list", rect, &list.title).show(ui, |ui| {
        {
            let (cell, _) = grid_named_row(ui, &tr!("delay-list-title"), 0);
            if cell.is_positive() {
                let taken = plan.delays().lists.iter().filter(|other| other.id != id).map(|other| other.title.clone());
                let problem = super::schedule_setup::name_problem(&draft.title, taken);
                let response = ui.put(cell, egui::TextEdit::singleline(&mut draft.title).desired_width(cell.width()));
                if let Some(message) = &problem {
                    response.clone().on_hover_text(message);
                }
                if response.lost_focus() && problem.is_none() && draft.title.trim() != list.title {
                    edits.push(UiCommand::schedule(
                        session,
                        ScheduleEdit::RenameDelayList {
                            list: id,
                            title: draft.title.trim().to_owned(),
                        },
                    ));
                }
            }
        }
        {
            let (cell, _) = grid_named_row(ui, &tr!("delay-type"), 0);
            if cell.is_positive() {
                let mut kind = list.kind;
                if combo_in(
                    ui,
                    egui::Id::new(("delay_list_type", id.0)),
                    cell,
                    &mut kind,
                    type_label(plan, list.kind),
                    type_options(plan),
                ) && kind != list.kind
                {
                    edits.push(UiCommand::schedule(session, ScheduleEdit::SetDelayListType { list: id, kind }));
                }
            }
        }

        grid_separator_row(ui, &tr!("delay-entries"), 0);
        header_row(ui, &fractions, 32.0, &[tr!("delay-machine"), tr!("delay-start"), tr!("delay-end"), tr!("delay-hours")]);
        if list.entries.is_empty() {
            explorer_note(ui, tr!("delay-list-empty"));
        }
        let mut remove = None;
        for (index, entry) in list.entries.iter().enumerate() {
            let (_, cells, end) = table_row(ui, &fractions, 32.0);
            let mut agent = entry.agent;
            if combo_in(
                ui,
                egui::Id::new(("delay_entry_agent", id.0, index)),
                cells[0],
                &mut agent,
                agent_name(plan, entry.agent),
                agents.clone(),
            ) && agent != entry.agent
            {
                entries[index].agent = agent;
                entries_changed = true;
            }
            for (column, cell) in [(0usize, cells[1]), (1, cells[2])] {
                let Some(text) = draft.cells.get_mut(index).map(|cells| &mut cells[column]) else {
                    continue;
                };
                let parsed = parse_instant(text);
                let response = ui.put(cell, egui::TextEdit::singleline(text).desired_width(cell.width()));
                if parsed.is_none() {
                    response.clone().on_hover_text(tr!("delay-instant-hint"));
                }
                if response.lost_focus()
                    && let Some(hours) = parsed
                {
                    let mut edited = *entry;
                    if column == 0 {
                        edited.start_h = hours;
                    } else {
                        edited.end_h = hours;
                    }
                    if edited != *entry && edited.is_valid() {
                        entries[index] = edited;
                        entries_changed = true;
                    }
                }
            }
            ui.put(
                cells[3],
                egui::Label::new(egui::RichText::new(hours_text(entry.end_h - entry.start_h)).color(ui.visuals().weak_text_color())).halign(egui::Align::Min),
            );
            if delete_button(ui, end, tr!("delay-delete-entry")) {
                remove = Some(index);
            }
        }
        if let Some(index) = remove {
            entries.remove(index);
            entries_changed = true;
        }

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.add_space(6.0);
            let add = ui.add_enabled(!agents.is_empty(), egui::Button::new(tr!("delay-add-entry")).corner_radius(GROUP_CORNER_RADIUS));
            if add.clicked() {
                // After the last row, on the same machine, for an hour: the
                // shape the next row most likely has.
                let previous = entries.last().copied();
                let start_h = previous.map_or(0.0, |entry| entry.end_h);
                entries.push(DelayEntry {
                    agent: previous.map_or(agents[0].0, |entry| entry.agent),
                    start_h,
                    end_h: start_h + 1.0,
                });
                entries_changed = true;
            }
            if ui.add(egui::Button::new(tr!("delay-paste-rows")).corner_radius(GROUP_CORNER_RADIUS)).clicked() {
                draft.paste = Some(String::new());
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.add_space(6.0);
            ui.label(egui::RichText::new(tr!("delay-paste-hint")).weak());
        });

        if let Some(text) = draft.paste.as_mut() {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.add_space(6.0);
                ui.add(
                    egui::TextEdit::multiline(text)
                        .desired_rows(6)
                        .desired_width(ui.available_width() - 12.0)
                        .hint_text(tr!("delay-paste-example")),
                );
            });
            let mut close = false;
            ui.horizontal(|ui| {
                ui.add_space(6.0);
                if ui.add(egui::Button::new(tr!("delay-paste-add")).corner_radius(GROUP_CORNER_RADIUS)).clicked() {
                    match parse_delay_rows(text, &agents) {
                        Ok(rows) => {
                            entries.extend(rows);
                            entries_changed = true;
                            close = true;
                        }
                        Err(problems) => paste_error = Some(problems.iter().map(|(line, problem)| problem_text(*line, problem)).collect::<Vec<_>>().join("\n")),
                    }
                }
                if ui.add(egui::Button::new(tr!("delay-paste-cancel")).corner_radius(GROUP_CORNER_RADIUS)).clicked() {
                    close = true;
                }
            });
            if close {
                draft.paste = None;
            }
        }
    });
    if let Some(message) = paste_error {
        crate::userspace_warn!("{}", message);
    }
    if entries_changed {
        edits.push(UiCommand::schedule(session, ScheduleEdit::SetDelayListEntries { list: id, entries }));
    }
    commands.append(&mut edits);
}

/// One roster: what repeats, on which machines, and how often.
fn draw_roster(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    editor: &mut EditorState,
    plan: &SchedulePlan,
    id: crate::model::schedule::RosterId,
    session: u32,
    commands: &mut Vec<UiCommand>,
) {
    let Some(roster) = plan.delays().roster(id) else { return };
    let source = format!("{roster:?}");
    let draft = draft_for(editor, DelaySelection::Roster(id), source, || DelayDraft {
        title: roster.name.clone(),
        roster: [
            instant_text(roster.first_start_h),
            hours_text(roster.duration_h),
            hours_text(roster.every_h),
            roster.until_h.map(instant_text).unwrap_or_default(),
        ],
        ..DelayDraft::default()
    });
    let mut edited: Roster = roster.clone();
    let mut changed = false;
    DataGrid::new("schedule_delay_roster", rect, &roster.name).show(ui, |ui| {
        explorer_note(ui, tr!("delay-roster-note"));
        {
            let (cell, _) = grid_named_row(ui, &tr!("planning-name"), 0);
            if cell.is_positive() {
                let taken = plan.delays().rosters.iter().filter(|other| other.id != id).map(|other| other.name.clone());
                let problem = super::schedule_setup::name_problem(&draft.title, taken);
                let response = ui.put(cell, egui::TextEdit::singleline(&mut draft.title).desired_width(cell.width()));
                if let Some(message) = &problem {
                    response.clone().on_hover_text(message);
                }
                if response.lost_focus() && problem.is_none() && draft.title.trim() != roster.name {
                    edited.name = draft.title.trim().to_owned();
                    changed = true;
                }
            }
        }
        {
            let (cell, _) = grid_named_row(ui, &tr!("delay-type"), 0);
            if cell.is_positive()
                && combo_in(
                    ui,
                    egui::Id::new(("roster_type", id.0)),
                    cell,
                    &mut edited.kind,
                    type_label(plan, roster.kind),
                    type_options(plan),
                )
            {
                changed |= edited.kind != roster.kind;
            }
        }
        // Text cells: first start, duration, repeat, until.
        let labels = [
            tr!("delay-roster-first"),
            tr!("delay-roster-duration"),
            tr!("delay-roster-every"),
            tr!("delay-roster-until"),
        ];
        for (index, label) in labels.iter().enumerate() {
            let (cell, _) = grid_named_row(ui, label, 0);
            if !cell.is_positive() {
                continue;
            }
            let text = &mut draft.roster[index];
            let parsed: Option<Option<f64>> = match index {
                0 => parse_instant(text).map(Some),
                1 => text
                    .trim()
                    .parse::<f64>()
                    .ok()
                    .filter(|value| value.is_finite() && *value >= MIN_ROSTER_DURATION_H)
                    .map(Some),
                2 => text.trim().parse::<f64>().ok().filter(|value| value.is_finite() && *value >= MIN_ROSTER_EVERY_H).map(Some),
                _ if text.trim().is_empty() => Some(None),
                _ => parse_instant(text).map(Some),
            };
            let mut field = egui::TextEdit::singleline(text).desired_width(cell.width());
            if index == 3 {
                field = field.hint_text(tr!("delay-roster-until-hint"));
            }
            let response = ui.put(cell, field);
            if parsed.is_none() {
                response.clone().on_hover_text(match index {
                    1 => tr!("delay-roster-duration-hint"),
                    2 => tr!("delay-roster-every-hint"),
                    _ => tr!("delay-instant-hint"),
                });
            }
            if response.lost_focus()
                && let Some(value) = parsed
            {
                match index {
                    0 => edited.first_start_h = value.unwrap_or(0.0),
                    1 => edited.duration_h = value.unwrap_or(1.0),
                    2 => edited.every_h = value.unwrap_or(24.0),
                    _ => edited.until_h = value,
                }
                changed = true;
            }
        }

        grid_separator_row(ui, &tr!("delay-roster-machines"), 0);
        let mut all = matches!(edited.loaders, LoaderSelection::All);
        let (cell, _) = grid_named_row(ui, &tr!("delay-roster-all"), 0);
        if cell.is_positive() && ui.put(cell, egui::Checkbox::without_text(&mut all)).changed() {
            edited.loaders = if all {
                LoaderSelection::All
            } else {
                LoaderSelection::Only(plan.agents().iter().map(|agent| agent.id).collect())
            };
            changed = true;
        }
        if let LoaderSelection::Only(selected) = &edited.loaders {
            let mut chosen = selected.clone();
            for agent in plan.agents() {
                let (cell, _) = grid_named_row(ui, &agent.name, 1);
                let mut on = chosen.contains(&agent.id);
                if cell.is_positive() && ui.put(cell, egui::Checkbox::without_text(&mut on)).changed() {
                    if on {
                        chosen.push(agent.id);
                        chosen.sort();
                    } else {
                        chosen.retain(|other| *other != agent.id);
                    }
                    changed = true;
                }
            }
            if !chosen.is_empty() {
                edited.loaders = LoaderSelection::Only(chosen);
            }
        }
    });
    if changed && edited != *roster {
        if edited.is_valid() {
            commands.push(UiCommand::schedule(session, ScheduleEdit::SetRoster(edited)));
        } else {
            crate::userspace_warn!("{}", tr!("delay-roster-invalid"));
            // Back to what is stored, rather than leaving text that was refused.
            editor.schedule_delay_draft = None;
        }
    }
}
