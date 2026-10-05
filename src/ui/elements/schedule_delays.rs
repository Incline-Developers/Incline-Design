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
        delays::{DelayRowProblem, MIN_ROSTER_DURATION_H, MIN_ROSTER_EVERY_H, hours_text, parse_delay_rows, parse_time},
    },
    ui::{
        EditorState,
        state::{DelayDraft, DelaySelection, ScheduleEdit, UiCommand},
        widgets::{
            context_menu::{ContextMenuAction, context_menu_popup},
            data_grid::{DataGrid, GridRow, PropertyRows, PropertyTable, grid_add_action_row, grid_row, grid_separator_row, property_table_height},
            explorer::{explorer_note, row_height},
            toolbar::GROUP_CORNER_RADIUS,
        },
    },
};

/// An instant as it is typed into a cell: a date and time once Day 1 has a
/// date, the Gantt's "Day 3, 06:00" before.
pub(crate) fn instant_text(hours: f64) -> String {
    super::schedule_periods::editable_time(hours)
}

/// What a typed instant reads as, dates included once Day 1 has one.
pub(crate) fn read_time(text: &str) -> Option<f64> {
    parse_time(text, super::schedule_periods::clock_start())
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

/// The left column: the delay types, the delay lists and the rosters, each a
/// group with its own row to add another. Any type row opens the one table
/// that edits them all.
pub(crate) fn draw_delay_index(ui: &mut egui::Ui, rect: egui::Rect, editor: &mut EditorState, plan: &SchedulePlan, session: u32, commands: &mut Vec<UiCommand>) {
    let delays = plan.delays();
    let mut selected = editor.schedule_selected_delay;
    if selected.is_none() {
        selected = Some(DelaySelection::Types);
    }
    DataGrid::new("schedule_delay_index", rect, &tr!("delay-step")).show(ui, |ui| {
        grid_separator_row(ui, &tr!("delay-types"), 0);
        for entry in &delays.types {
            if grid_row(ui, GridRow::new(&entry.name).selected(selected == Some(DelaySelection::Types))).clicked() {
                selected = Some(DelaySelection::Types);
            }
        }
        if grid_add_action_row(ui, &tr!("delay-new-type")) {
            commands.push(new_type(plan, session));
            selected = Some(DelaySelection::Types);
        }

        grid_separator_row(ui, &tr!("delay-lists"), 0);
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
        if grid_add_action_row(ui, &tr!("delay-new-list")) {
            let title = crate::model::schedule::suggested_name(&tr!("delay-list-default"), delays.lists.iter().map(|list| list.title.clone()));
            commands.push(UiCommand::schedule(session, ScheduleEdit::AddDelayList { title }));
        }

        grid_separator_row(ui, &tr!("delay-rosters"), 0);
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
        if grid_add_action_row(ui, &tr!("delay-new-roster")) {
            let name = crate::model::schedule::suggested_name(&tr!("delay-roster-default"), delays.rosters.iter().map(|roster| roster.name.clone()));
            commands.push(UiCommand::schedule(session, ScheduleEdit::AddRoster { name }));
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

/// Add a delay type with the next free name and colour.
fn new_type(plan: &SchedulePlan, session: u32) -> UiCommand {
    let types = &plan.delays().types;
    let name = crate::model::schedule::suggested_name(&tr!("delay-type-default"), types.iter().map(|entry| entry.name.clone()));
    UiCommand::schedule(
        session,
        ScheduleEdit::AddDelayType {
            name,
            color: plan.delays().suggested_color(),
        },
    )
}

/// The Delay Type row of a list or roster. With no types to choose from it
/// says where they are made, which is not on this page.
fn type_row(rows: &mut PropertyRows<'_>, plan: &SchedulePlan, id: impl std::hash::Hash + std::fmt::Debug, kind: &mut Option<DelayTypeId>) -> bool {
    let current = *kind;
    let changed = rows.combo(id, &tr!("delay-type"), kind, &type_label(plan, current), type_options(plan)).changed();
    if plan.delays().types.is_empty() {
        rows.note(&tr!("delay-no-types-note"));
    }
    changed && *kind != current
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
        if grid_add_action_row(ui, &tr!("delay-new-type")) {
            edits.push(new_type(plan, session));
        }
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
            match parse_delay_rows(&text, &agents, super::schedule_periods::clock_start()) {
                Ok(rows) => {
                    entries.extend(rows);
                    entries_changed = true;
                }
                Err(problems) => paste_error = Some(problems.iter().map(|(line, problem)| problem_text(*line, problem)).collect::<Vec<_>>().join("\n")),
            }
        }
    }

    let no_types = plan.delays().types.is_empty();
    let table_rect = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), property_table_height(ui, 3 + usize::from(no_types)).min(rect.height())));
    PropertyTable::new("schedule_delay_list_properties", table_rect, &list.title).show(ui, |rows| {
        rows.header(&tr!("planning-property"), &tr!("planning-value"));
        let taken = plan.delays().lists.iter().filter(|other| other.id != id).map(|other| other.title.clone());
        let problem = super::schedule_setup::name_problem(&draft.title, taken);
        let response = rows.field(&tr!("delay-list-title"), &mut draft.title, problem.as_deref());
        if response.lost_focus() && problem.is_none() && draft.title.trim() != list.title {
            edits.push(UiCommand::schedule(
                session,
                ScheduleEdit::RenameDelayList {
                    list: id,
                    title: draft.title.trim().to_owned(),
                },
            ));
        }
        let mut kind = list.kind;
        if type_row(rows, plan, ("delay_list_type", id.0), &mut kind) {
            edits.push(UiCommand::schedule(session, ScheduleEdit::SetDelayListType { list: id, kind }));
        }
    });
    let below = egui::Rect::from_min_max(egui::pos2(rect.left(), table_rect.bottom() + ui.spacing().item_spacing.y), rect.max);
    let fractions = [0.34, 0.27, 0.27, 0.12];
    let (paste_label, paste_hint) = (tr!("delay-paste-rows"), tr!("delay-paste-hint"));
    let mut open_paste = false;
    DataGrid::new("schedule_delay_list", below, &tr!("delay-entries"))
        .title_action(&paste_label, &paste_hint, &mut open_paste)
        .show(ui, |ui| {
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
                    let parsed = read_time(text);
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

            // A delay needs a machine to stop, so there is none to add until
            // the fleet has one.
            if !agents.is_empty() && grid_add_action_row(ui, &tr!("delay-add-entry")) {
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
                        match parse_delay_rows(text, &agents, super::schedule_periods::clock_start()) {
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
    if open_paste {
        draft.paste = Some(String::new());
    }
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
    PropertyTable::new("schedule_delay_roster", rect, &roster.name).show(ui, |rows| {
        rows.header(&tr!("planning-property"), &tr!("planning-value"));
        rows.note(&tr!("delay-roster-note"));
        let taken = plan.delays().rosters.iter().filter(|other| other.id != id).map(|other| other.name.clone());
        let problem = super::schedule_setup::name_problem(&draft.title, taken);
        let response = rows.field(&tr!("planning-name"), &mut draft.title, problem.as_deref());
        if response.lost_focus() && problem.is_none() && draft.title.trim() != roster.name {
            edited.name = draft.title.trim().to_owned();
            changed = true;
        }
        changed |= type_row(rows, plan, ("roster_type", id.0), &mut edited.kind);
        // Text cells: first start, duration, repeat, until.
        let labels = [
            tr!("delay-roster-first"),
            tr!("delay-roster-duration"),
            tr!("delay-roster-every"),
            tr!("delay-roster-until"),
        ];
        for (index, label) in labels.iter().enumerate() {
            let text = &mut draft.roster[index];
            let parsed: Option<Option<f64>> = match index {
                0 => read_time(text).map(Some),
                1 => text
                    .trim()
                    .parse::<f64>()
                    .ok()
                    .filter(|value| value.is_finite() && *value >= MIN_ROSTER_DURATION_H)
                    .map(Some),
                2 => text.trim().parse::<f64>().ok().filter(|value| value.is_finite() && *value >= MIN_ROSTER_EVERY_H).map(Some),
                _ if text.trim().is_empty() => Some(None),
                _ => read_time(text).map(Some),
            };
            let error = parsed.is_none().then(|| match index {
                1 => tr!("delay-roster-duration-hint"),
                2 => tr!("delay-roster-every-hint"),
                _ => tr!("delay-instant-hint"),
            });
            let response = if index == 3 {
                rows.field_with_hint(label, text, &tr!("delay-roster-until-hint"), error.as_deref())
            } else {
                rows.field(label, text, error.as_deref())
            };
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

        rows.header(&tr!("delay-roster-machines"), "");
        let mut all = matches!(edited.loaders, LoaderSelection::All);
        if rows.checkbox(&tr!("delay-roster-all"), &mut all).changed() {
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
                let mut on = chosen.contains(&agent.id);
                if rows.checkbox(&agent.name, &mut on).changed() {
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
