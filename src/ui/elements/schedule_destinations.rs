//! The Schedule workspace's destination pages: Stockpiles, Dumps, Crushers and
//! the ordered routing rules.
//!
//! Three lists and a rule table. The lists are *derived*: every stockpile and
//! dump solid the project holds appears on its page automatically, named by
//! the solid, so drawing one in Solids is all it takes to deliver to it and
//! there is no second name to keep in step. What these pages own is the
//! receiving capacity, and - for standalone destinations with no geometry -
//! the name and kind as well.
//!
//! Nothing here computes and nothing here writes on open: a page that created
//! a document entry merely by being looked at would put an undo step on the
//! stack nobody asked for.

use crate::{
    i18n::{tr, tr_format},
    model::{
        Document, ReserveAggregation,
        schedule::{Bound, ConditionTest, DestinationId, DestinationKind, FieldCondition, LoaderSelection, SchedulePlan, SourceSelection, StandaloneDestinationId, destinations},
    },
    ui::{
        EditorState,
        state::{ScheduleConditionDraft, ScheduleDestinationDraft, ScheduleEdit, ScheduleRuleDraft, UiCommand},
        widgets::{
            context_menu::{ContextMenuAction, context_menu_popup},
            data_grid::{DataGrid, GridRow, PropertyTable, grid_choice_row, grid_row, grid_separator_row, property_table_height},
            explorer::explorer_note,
            menu::{self, DragableMenu, MenuButton, MenuFieldCombo, MenuFieldText},
        },
    },
};

/// Blank is unlimited, and is not the same answer as zero - so the field holds
/// text and an empty one is a deliberate choice rather than a failed parse.
fn capacity_text(capacity: Option<f64>) -> String {
    capacity.map(|value| value.to_string()).unwrap_or_default()
}

/// Parse a typed capacity. An empty field is unlimited; anything else must be a
/// number of tonnes that is not negative.
fn parse_capacity(text: &str) -> Result<Option<f64>, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let Ok(value) = trimmed.parse::<f64>() else {
        return Err(tr!("schedule-rate-not-a-number"));
    };
    if !value.is_finite() || value < 0.0 {
        return Err(crate::model::schedule::ScheduleError::InvalidCapacity.message());
    }
    Ok(Some(value))
}

fn name_problem(name: &str, taken: impl Iterator<Item = String>) -> Option<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Some(crate::model::schedule::ScheduleError::EmptyName.message());
    }
    taken
        .into_iter()
        .any(|other| other.trim().eq_ignore_ascii_case(trimmed))
        .then(|| crate::model::schedule::ScheduleError::DuplicateName(trimmed.to_owned()).message())
}

/// The destinations of one kind, solid-backed first and then standalone.
///
/// Rebuilt from the document every frame on purpose: a stockpile drawn while
/// this page was open should be in the list, and nothing has to notice it was
/// added for that to happen.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_destination_list(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    editor: &mut EditorState,
    plan: &SchedulePlan,
    document: &Document,
    kind: DestinationKind,
    session: u32,
    commands: &mut Vec<UiCommand>,
) {
    let available = destinations::available(document.solids(), plan.routing());
    let entries: Vec<_> = available.into_iter().filter(|entry| entry.kind == kind).collect();
    let mut selected = editor.schedule_selected_destination;
    let mut open_dialog = false;
    let title = kind_page_title(kind);
    DataGrid::new(list_id(kind), rect, &title).column_header(&tr!("planning-name")).show(ui, |ui| {
        if entries.is_empty() {
            explorer_note(ui, empty_note(kind));
        }
        for entry in &entries {
            // The capacity beside the name, because a destination that can
            // receive nothing and one with room are different things to route
            // to and the list is where that is compared.
            let label = match entry.kind {
                DestinationKind::Crusher => match plan.routing().crusher(standalone_of(entry.id)).and_then(|calendar| calendar.default_tpd) {
                    Some(limit) => tr_format!(literal = "%name% · %limit%", name = entry.name.clone(), limit = tonnes_per_day(limit)),
                    None => tr_format!(literal = "%name% · %limit%", name = entry.name.clone(), limit = tr!("destination-unlimited")),
                },
                _ => match entry.capacity_t {
                    Some(capacity) => tr_format!(literal = "%name% · %limit%", name = entry.name.clone(), limit = tonnes(capacity)),
                    None => tr_format!(literal = "%name% · %limit%", name = entry.name.clone(), limit = tr!("destination-unlimited")),
                },
            };
            let response = grid_row(ui, GridRow::new(&label).selected(selected == Some(entry.id))).on_hover_text(&label);
            if response.clicked() {
                selected = Some(entry.id);
            }
            let linked = entry.is_linked();
            context_menu_popup(&response, &entry.name, |ui| {
                // A linked destination is the solid: it is deleted in Solids,
                // and offering deletion here would imply the schedule owned it.
                if ContextMenuAction::new(tr!("destination-delete")).enabled(!linked).show(ui).clicked() {
                    if let DestinationId::Standalone(id) = entry.id {
                        commands.push(UiCommand::schedule(session, ScheduleEdit::DeleteDestination(id)));
                    }
                    ui.close();
                }
                if linked {
                    ContextMenuAction::new(tr!("destination-edit-in-solids")).enabled(false).show(ui);
                }
            });
        }
        let body = ui.available_rect_before_wrap();
        if body.is_positive() {
            let response = ui.interact(body, ui.id().with(("new_destination_space", list_id(kind))), egui::Sense::click());
            context_menu_popup(&response, title.clone(), |ui| {
                if ContextMenuAction::new(new_label(kind)).show(ui).clicked() {
                    open_dialog = true;
                    ui.close();
                }
            });
        }
    });
    if editor.schedule_selected_destination != selected {
        editor.schedule_destination_draft = None;
    }
    editor.schedule_selected_destination = selected;
    if open_dialog && !editor.new_destination_open {
        editor.new_destination_name = suggested_destination_name(plan, document, kind);
        editor.new_destination_kind = kind;
        editor.new_destination_open = true;
    }
}

/// The selected destination's cells.
///
/// A linked destination's name and type are read-only and say where they are
/// edited; its capacity is the schedule's own and is editable here. A crusher
/// has no storage in this increment, so it carries a daily budget instead of a
/// capacity.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_destination_properties(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    editor: &mut EditorState,
    plan: &SchedulePlan,
    document: &Document,
    kind: DestinationKind,
    session: u32,
    commands: &mut Vec<UiCommand>,
) {
    let selected = editor
        .schedule_selected_destination
        .and_then(|id| destinations::resolve(id, document.solids(), plan.routing()).ok())
        .filter(|entry| entry.kind == kind);
    let Some(entry) = selected else {
        PropertyTable::new("schedule_destination_properties", rect, &kind_page_title(kind)).show(ui, |rows| {
            rows.header(&tr!("planning-property"), &tr!("planning-value"));
            rows.readonly(&tr!("planning-name"), &tr!("destination-select"), None, None);
        });
        return;
    };
    let crusher_default = plan
        .routing()
        .crusher(entry.solid.map_or_else(|| standalone_of(entry.id), |_| standalone_of(entry.id)))
        .and_then(|calendar| calendar.default_tpd);
    let crusher_default = if entry.kind == DestinationKind::Crusher { crusher_default } else { None };
    let source = (entry.name.clone(), entry.capacity_t, crusher_default);
    if editor
        .schedule_destination_draft
        .as_ref()
        .is_none_or(|draft| draft.id != entry.id || draft.source != source)
    {
        editor.schedule_destination_draft = Some(ScheduleDestinationDraft {
            id: entry.id,
            name: entry.name.clone(),
            capacity: capacity_text(entry.capacity_t),
            crusher_default: capacity_text(crusher_default),
            source,
        });
    }
    let draft = editor.schedule_destination_draft.as_mut().expect("just ensured");
    let linked = entry.solid.is_some();
    let taken: Vec<String> = plan
        .routing()
        .standalone
        .iter()
        .filter(|other| DestinationId::Standalone(other.id) != entry.id)
        .map(|other| other.name.clone())
        .collect();
    let name_error = (!linked).then(|| name_problem(&draft.name, taken.iter().cloned())).flatten();
    let capacity_error = parse_capacity(&draft.capacity).err();
    let crusher_error = parse_capacity(&draft.crusher_default).err();
    let rows_used = 4 + usize::from(linked);
    let table_rect = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), property_table_height(ui, rows_used).min(rect.height())));
    let mut edits = Vec::new();
    PropertyTable::new("schedule_destination_properties", table_rect, &entry.name).show(ui, |rows| {
        rows.header(&tr!("planning-property"), &tr!("planning-value"));
        if linked {
            rows.readonly(&tr!("planning-name"), &entry.name, None, None);
            rows.readonly(&tr!("destination-linked-solid"), &tr!("destination-linked-note"), None, None);
        } else {
            let response = rows.field(&tr!("planning-name"), &mut draft.name, name_error.as_deref());
            if response.lost_focus()
                && name_error.is_none()
                && draft.name.trim() != entry.name
                && let DestinationId::Standalone(id) = entry.id
            {
                edits.push(UiCommand::schedule(
                    session,
                    ScheduleEdit::RenameDestination {
                        destination: id,
                        name: draft.name.trim().to_owned(),
                    },
                ));
            }
        }
        rows.readonly(&tr!("destination-type"), &entry.kind.label(), None, None);
        if entry.kind == DestinationKind::Crusher {
            let response = rows.field(&tr!("destination-daily-limit"), &mut draft.crusher_default, crusher_error.as_deref());
            if response.lost_focus()
                && let Ok(limit) = parse_capacity(&draft.crusher_default)
                && limit != crusher_default
                && let DestinationId::Standalone(id) = entry.id
            {
                edits.push(UiCommand::schedule(
                    session,
                    ScheduleEdit::SetCrusherCells {
                        edits: vec![crate::model::schedule::CrusherCellEdit {
                            destination: id,
                            cell: crate::model::schedule::CrusherCell::Default,
                            value: limit.map(crate::model::schedule::CrusherOverride::Limit),
                        }],
                    },
                ));
            }
        } else {
            let response = rows.field(&tr!("destination-capacity"), &mut draft.capacity, capacity_error.as_deref());
            if response.lost_focus()
                && let Ok(capacity) = parse_capacity(&draft.capacity)
                && capacity != entry.capacity_t
            {
                edits.push(UiCommand::schedule(
                    session,
                    ScheduleEdit::SetDestinationCapacity {
                        destination: entry.id,
                        capacity_t: capacity,
                    },
                ));
            }
        }
    });
    commands.append(&mut edits);
}

/// The rule table: every rule in priority order, with what it matches.
///
/// Compact and ordered, not a graphical builder: the order *is* the resolution,
/// so the one thing the table has to make obvious is which rule comes first.
pub(crate) fn draw_rule_list(ui: &mut egui::Ui, rect: egui::Rect, editor: &mut EditorState, plan: &SchedulePlan, document: &Document, session: u32, commands: &mut Vec<UiCommand>) {
    let routing = plan.routing();
    let mut selected = editor.schedule_selected_rule;
    let mut new_rule = false;
    DataGrid::new("schedule_rule_list", rect, &tr!("destination-destinations"))
        .column_header(&tr!("destination-rule-order"))
        .show(ui, |ui| {
            if !routing.enabled {
                explorer_note(ui, tr!("destination-routing-off-note"));
            }
            if routing.rules.is_empty() {
                explorer_note(ui, tr!("destination-no-rules"));
            }
            for (position, rule) in routing.rules.iter().enumerate() {
                let destination = destinations::resolve(rule.destination, document.solids(), routing);
                let target = destination.as_ref().map_or_else(|_| tr!("destination-unresolved"), |entry| entry.name.clone());
                let label = tr_format!(
                    literal = "%order%. %name% → %target%",
                    order = (position + 1).to_string(),
                    name = rule.name.clone(),
                    target = target
                );
                let summary = rule.summary(
                    |agent| plan.agent(agent).map(|agent| agent.name.clone()).unwrap_or_else(|| tr!("schedule-error-unknown-agent")),
                    |field| {
                        document
                            .reserve_fields()
                            .iter()
                            .find(|entry| entry.id == field)
                            .map(|entry| entry.name.clone())
                            .unwrap_or_else(|| tr!("destination-stage-field-missing"))
                    },
                );
                // A disabled rule is dimmed rather than hidden: it is part of
                // the order the user is reading, and hiding it would make the
                // numbering jump.
                let disabled = (!rule.enabled).then(|| tr!("destination-rule-disabled"));
                let row = GridRow::new(&label).error(disabled.as_deref());
                let response =
                    grid_row(ui, row.selected(selected == Some(rule.id))).on_hover_text(tr_format!(literal = "%label%\n%summary%", label = label.clone(), summary = summary));
                if response.clicked() {
                    selected = Some(rule.id);
                }
                context_menu_popup(&response, &rule.name, |ui| {
                    if ContextMenuAction::new(tr!("destination-move-rule-up")).enabled(position > 0).show(ui).clicked() {
                        commands.push(UiCommand::schedule(session, ScheduleEdit::MoveRule { rule: rule.id, later: false }));
                        ui.close();
                    }
                    if ContextMenuAction::new(tr!("destination-move-rule-down"))
                        .enabled(position + 1 < routing.rules.len())
                        .show(ui)
                        .clicked()
                    {
                        commands.push(UiCommand::schedule(session, ScheduleEdit::MoveRule { rule: rule.id, later: true }));
                        ui.close();
                    }
                    if ContextMenuAction::new(tr!("destination-duplicate-rule")).show(ui).clicked() {
                        commands.push(UiCommand::schedule(session, ScheduleEdit::DuplicateRule(rule.id)));
                        ui.close();
                    }
                    if ContextMenuAction::new(tr!("destination-delete-rule")).show(ui).clicked() {
                        commands.push(UiCommand::schedule(session, ScheduleEdit::DeleteRule(rule.id)));
                        ui.close();
                    }
                });
            }
            let body = ui.available_rect_before_wrap();
            if body.is_positive() {
                let response = ui.interact(body, ui.id().with("new_rule_space"), egui::Sense::click());
                context_menu_popup(&response, tr!("destination-destinations"), |ui| {
                    let available = !destinations::available(document.solids(), routing).is_empty();
                    if ContextMenuAction::new(tr!("destination-new-rule")).enabled(available).show(ui).clicked() {
                        new_rule = true;
                        ui.close();
                    }
                });
            }
        });
    if editor.schedule_selected_rule != selected {
        editor.schedule_rule_draft = None;
        editor.schedule_condition_draft = None;
    }
    editor.schedule_selected_rule = selected;
    if new_rule && let Some(first) = destinations::available(document.solids(), routing).first() {
        let name = crate::model::schedule::suggested_name(&tr!("destination-rule-default"), routing.rules.iter().map(|rule| rule.name.clone()));
        commands.push(UiCommand::schedule(session, ScheduleEdit::AddRule { name, destination: first.id }));
    }
}

/// The selected rule's editor: what it delivers to, and the three filter
/// groups that decide what reaches it.
pub(crate) fn draw_rule_editor(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    editor: &mut EditorState,
    plan: &SchedulePlan,
    document: &Document,
    session: u32,
    commands: &mut Vec<UiCommand>,
) {
    let routing = plan.routing();
    let Some(rule) = editor.schedule_selected_rule.and_then(|id| routing.rule(id)).cloned() else {
        PropertyTable::new("schedule_rule_editor", rect, &tr!("destination-rule")).show(ui, |rows| {
            rows.header(&tr!("planning-property"), &tr!("planning-value"));
            rows.readonly(&tr!("planning-name"), &tr!("destination-select-rule"), None, None);
        });
        return;
    };
    if editor.schedule_rule_draft.as_ref().is_none_or(|draft| draft.id != rule.id || draft.source != rule.name) {
        editor.schedule_rule_draft = Some(ScheduleRuleDraft {
            id: rule.id,
            source: rule.name.clone(),
            name: rule.name.clone(),
        });
    }
    let available = destinations::available(document.solids(), routing);
    let sources = editor.schedule_routing_sources.clone();
    let categories = editor.schedule_category_values.clone();
    let mut edits = Vec::new();
    let taken: Vec<String> = routing.rules.iter().filter(|other| other.id != rule.id).map(|other| other.name.clone()).collect();
    let draft = editor.schedule_rule_draft.as_mut().expect("just ensured");
    let name_error = name_problem(&draft.name, taken.iter().cloned());
    let mut condition_action = None;
    DataGrid::new("schedule_rule_editor", rect, &rule.name)
        .column_header(&tr!("destination-rule"))
        .show(ui, |ui| {
            grid_separator_row(ui, &tr!("destination-rule"), 0);
            {
                let (cell, _) = crate::ui::widgets::data_grid::grid_named_row(ui, &tr!("planning-name"), 0);
                if cell.is_positive() {
                    let response = ui.put(cell, egui::TextEdit::singleline(&mut draft.name).desired_width(cell.width()));
                    if let Some(message) = &name_error {
                        response.clone().on_hover_text(message);
                    }
                    if response.lost_focus() && name_error.is_none() && draft.name.trim() != rule.name {
                        edits.push(UiCommand::schedule(
                            session,
                            ScheduleEdit::RenameRule {
                                rule: rule.id,
                                name: draft.name.trim().to_owned(),
                            },
                        ));
                    }
                }
            }
            let mut enabled = rule.enabled;
            if crate::ui::widgets::data_grid::grid_checkbox_row(ui, &tr!("destination-rule-enabled"), &mut enabled, 0) {
                edits.push(UiCommand::schedule(session, ScheduleEdit::SetRuleEnabled { rule: rule.id, enabled }));
            }
            let mut destination = rule.destination;
            if !available.is_empty()
                && grid_choice_row(
                    ui,
                    ("rule_destination", rule.id.0),
                    &tr!("destination-rule-target"),
                    &mut destination,
                    available.iter().map(|entry| entry.id).collect::<Vec<_>>(),
                    |id| {
                        available
                            .iter()
                            .find(|entry| entry.id == id)
                            .map(|entry| tr_format!(literal = "%name% · %kind%", name = entry.name.clone(), kind = entry.kind.label()))
                            .unwrap_or_else(|| tr!("destination-unresolved"))
                    },
                    0,
                )
                && destination != rule.destination
            {
                edits.push(UiCommand::schedule(session, ScheduleEdit::SetRuleDestination { rule: rule.id, destination }));
            }

            // Loaders. "All" is not the same as "every loader currently in the
            // fleet": a machine added tomorrow is covered by All and is not added
            // to a list of names.
            grid_separator_row(ui, &tr!("destination-rule-loaders"), 0);
            let mut all_loaders = matches!(rule.loaders, LoaderSelection::All);
            if crate::ui::widgets::data_grid::grid_checkbox_row(ui, &tr!("destination-rule-all-loaders"), &mut all_loaders, 1) {
                let loaders = if all_loaders {
                    LoaderSelection::All
                } else {
                    LoaderSelection::Only(plan.agents().first().map(|agent| vec![agent.id]).unwrap_or_default())
                };
                // Refused when the fleet is empty rather than stored as an empty
                // list, which would match nothing while reading as a restriction.
                if !matches!(&loaders, LoaderSelection::Only(agents) if agents.is_empty()) {
                    edits.push(UiCommand::schedule(session, ScheduleEdit::SetRuleLoaders { rule: rule.id, loaders }));
                }
            }
            if let LoaderSelection::Only(chosen) = &rule.loaders {
                for agent in plan.agents() {
                    let mut picked = chosen.contains(&agent.id);
                    if crate::ui::widgets::data_grid::grid_checkbox_row(ui, &agent.name, &mut picked, 2) {
                        let mut next = chosen.clone();
                        if picked {
                            next.push(agent.id);
                        } else {
                            next.retain(|held| *held != agent.id);
                        }
                        if next.is_empty() {
                            // Clearing the last one means All, which is what the
                            // user is asking for and is a state that can be stored.
                            edits.push(UiCommand::schedule(
                                session,
                                ScheduleEdit::SetRuleLoaders {
                                    rule: rule.id,
                                    loaders: LoaderSelection::All,
                                },
                            ));
                        } else {
                            edits.push(UiCommand::schedule(
                                session,
                                ScheduleEdit::SetRuleLoaders {
                                    rule: rule.id,
                                    loaders: LoaderSelection::Only(next),
                                },
                            ));
                        }
                    }
                }
            }

            // Sources: the bands the last completed Solids run produced, nested
            // pit → bench → flitch. Scopes may be mixed freely.
            grid_separator_row(ui, &tr!("destination-rule-sources"), 0);
            let mut all_sources = matches!(rule.sources, SourceSelection::All);
            if crate::ui::widgets::data_grid::grid_checkbox_row(ui, &tr!("destination-rule-all-sources"), &mut all_sources, 1) {
                let next = if all_sources {
                    Some(SourceSelection::All)
                } else {
                    sources.first().map(|view| SourceSelection::Only(vec![view.scope]))
                };
                if let Some(sources) = next {
                    edits.push(UiCommand::schedule(session, ScheduleEdit::SetRuleSources { rule: rule.id, sources }));
                }
            }
            if let SourceSelection::Only(chosen) = &rule.sources {
                if sources.is_empty() {
                    explorer_note(ui, tr!("destination-no-sources"));
                }
                for view in sources.iter() {
                    let mut picked = chosen.contains(&view.scope);
                    if crate::ui::widgets::data_grid::grid_checkbox_row(ui, &view.label, &mut picked, view.depth + 2) {
                        let mut next = chosen.clone();
                        if picked {
                            next.push(view.scope);
                        } else {
                            next.retain(|held| *held != view.scope);
                        }
                        let sources = if next.is_empty() { SourceSelection::All } else { SourceSelection::Only(next) };
                        edits.push(UiCommand::schedule(session, ScheduleEdit::SetRuleSources { rule: rule.id, sources }));
                    }
                }
                // The scopes the run cannot place. Kept in the rule and named here
                // rather than dropped: the run may be stale, and a rule that
                // quietly stopped restricting would route material elsewhere.
                for scope in chosen.iter().filter(|scope| !sources.iter().any(|view| view.scope == **scope)) {
                    let _ = scope;
                    explorer_note(ui, tr!("destination-source-unplaced"));
                }
            }

            // Conditions, ANDed. A field carries at most one, because one interval
            // or value set already expresses anything two on the same field could.
            grid_separator_row(ui, &tr!("destination-rule-conditions"), 0);
            if rule.conditions.is_empty() {
                explorer_note(ui, tr!("destination-no-conditions"));
            }
            for condition in &rule.conditions {
                let field_name = document
                    .reserve_fields()
                    .iter()
                    .find(|field| field.id == condition.field)
                    .map(|field| field.name.clone())
                    .unwrap_or_else(|| tr!("destination-stage-field-missing"));
                let label = condition.summary(&field_name);
                let response = grid_row(ui, GridRow::new(&label)).on_hover_text(condition_note(document, condition));
                context_menu_popup(&response, &label, |ui| {
                    if ContextMenuAction::new(tr!("destination-edit-condition")).show(ui).clicked() {
                        condition_action = Some(ConditionAction::Edit(condition.field));
                        ui.close();
                    }
                    if ContextMenuAction::new(tr!("destination-delete-condition")).show(ui).clicked() {
                        condition_action = Some(ConditionAction::Delete(condition.field));
                        ui.close();
                    }
                });
            }
            let body = ui.available_rect_before_wrap();
            if body.is_positive() {
                let response = ui.interact(body, ui.id().with(("new_condition_space", rule.id.0)), egui::Sense::click());
                context_menu_popup(&response, tr!("destination-rule-conditions"), |ui| {
                    let spare = document
                        .reserve_fields()
                        .iter()
                        .any(|field| !rule.conditions.iter().any(|condition| condition.field == field.id));
                    if ContextMenuAction::new(tr!("destination-add-condition")).enabled(spare).show(ui).clicked() {
                        condition_action = Some(ConditionAction::Add);
                        ui.close();
                    }
                });
            }
        });
    match condition_action {
        None => {}
        Some(ConditionAction::Add) => {
            editor.schedule_condition_draft = Some(ScheduleConditionDraft {
                rule: rule.id,
                replacing: None,
                field: document
                    .reserve_fields()
                    .iter()
                    .find(|field| !rule.conditions.iter().any(|condition| condition.field == field.id))
                    .map(|field| field.id),
                values: Vec::new(),
                lower: String::new(),
                lower_inclusive: false,
                upper: String::new(),
                upper_inclusive: false,
            });
        }
        Some(ConditionAction::Edit(field)) => {
            let condition = rule.conditions.iter().find(|condition| condition.field == field);
            let (values, lower, lower_inclusive, upper, upper_inclusive) = match condition.map(|condition| &condition.test) {
                Some(ConditionTest::Category { values }) => (values.clone(), String::new(), false, String::new(), false),
                Some(ConditionTest::Range { lower, upper }) => (
                    Vec::new(),
                    lower.map(|bound| bound.value.to_string()).unwrap_or_default(),
                    lower.is_some_and(|bound| bound.inclusive),
                    upper.map(|bound| bound.value.to_string()).unwrap_or_default(),
                    upper.is_some_and(|bound| bound.inclusive),
                ),
                None => (Vec::new(), String::new(), false, String::new(), false),
            };
            editor.schedule_condition_draft = Some(ScheduleConditionDraft {
                rule: rule.id,
                replacing: Some(field),
                field: Some(field),
                values,
                lower,
                lower_inclusive,
                upper,
                upper_inclusive,
            });
        }
        Some(ConditionAction::Delete(field)) => {
            let conditions = rule.conditions.iter().filter(|condition| condition.field != field).cloned().collect();
            edits.push(UiCommand::schedule(session, ScheduleEdit::SetRuleConditions { rule: rule.id, conditions }));
        }
    }
    commands.append(&mut edits);
    draw_condition_dialog(ui, editor, plan, document, &categories, session, commands);
}

enum ConditionAction {
    Add,
    Edit(crate::model::ReserveFieldId),
    Delete(crate::model::ReserveFieldId),
}

/// What a condition is read against, stated on demand rather than on the page.
///
/// A summed field's condition compares the *contributing row's own* mapped
/// value, not the tonnes that row contributed - which is the one thing about
/// this that is not obvious from the expression.
fn condition_note(document: &Document, condition: &FieldCondition) -> String {
    let aggregation = document.reserve_fields().iter().find(|field| field.id == condition.field).map(|field| field.aggregation);
    match aggregation {
        Some(ReserveAggregation::Sum) => tr!("destination-condition-note-sum"),
        Some(ReserveAggregation::WeightedAverage { .. }) => tr!("destination-condition-note-average"),
        Some(ReserveAggregation::Category) => tr!("destination-condition-note-category"),
        None => tr!("destination-stage-field-missing"),
    }
}

/// The condition editor: one field, and either the values it may hold or the
/// interval it must fall in.
fn draw_condition_dialog(
    ui: &mut egui::Ui,
    editor: &mut EditorState,
    plan: &SchedulePlan,
    document: &Document,
    categories: &std::collections::BTreeMap<crate::model::ReserveFieldId, Vec<String>>,
    session: u32,
    commands: &mut Vec<UiCommand>,
) {
    let Some(draft) = editor.schedule_condition_draft.as_mut() else { return };
    let rule_id = draft.rule;
    let Some(rule) = plan.routing().rule(rule_id).cloned() else {
        editor.schedule_condition_draft = None;
        return;
    };
    let mut open = true;
    let mut close = false;
    let mut apply = false;
    let field = draft.field.and_then(|id| document.reserve_fields().iter().find(|field| field.id == id).cloned());
    let categorical = field.as_ref().is_some_and(|field| field.aggregation == ReserveAggregation::Category);
    DragableMenu::new("destination_condition_dialog", tr!("destination-condition"))
        .open(&mut open)
        .min_width(360.0)
        .show(ui.ctx(), |ui| {
            let options: Vec<(Option<crate::model::ReserveFieldId>, egui::WidgetText)> = document
                .reserve_fields()
                .iter()
                // A field that already carries a condition on this rule is not
                // offered: two would be ANDed, and one condition can already
                // say whatever both would.
                .filter(|entry| !rule.conditions.iter().any(|condition| condition.field == entry.id) || draft.replacing == Some(entry.id))
                .map(|entry| (Some(entry.id), entry.name.clone().into()))
                .collect();
            let selected = field.as_ref().map(|field| field.name.clone()).unwrap_or_else(|| tr!("destination-condition-pick-field"));
            MenuFieldCombo::new("destination_condition_field", tr!("destination-condition-field"), &mut draft.field, selected, options).show(ui);
            if categorical {
                menu::menu_section(ui, tr!("destination-condition-values"));
                let known = draft.field.and_then(|id| categories.get(&id));
                match known {
                    None => menu::menu_note(ui, tr!("destination-condition-no-values")),
                    Some(values) => {
                        for value in values {
                            let mut picked = draft.values.iter().any(|held| held == value);
                            if ui.checkbox(&mut picked, value).changed() {
                                if picked {
                                    draft.values.push(value.clone());
                                } else {
                                    draft.values.retain(|held| held != value);
                                }
                            }
                        }
                    }
                }
                // Selections the models no longer hold, still listed so they can
                // be seen and removed rather than silently narrowing the rule.
                let retained: Vec<String> = draft
                    .values
                    .iter()
                    .filter(|value| !known.is_some_and(|values| values.iter().any(|entry| entry == *value)))
                    .cloned()
                    .collect();
                for value in retained {
                    let mut picked = true;
                    let label = tr_format!(literal = "%value% (%note%)", value = value.clone(), note = tr!("destination-condition-absent"));
                    if ui.checkbox(&mut picked, label).changed() {
                        draft.values.retain(|held| *held != value);
                    }
                }
            } else {
                // Both ends, each with its own inclusivity, so `60 < Fe < 70` is
                // expressible exactly as it was written.
                menu::menu_section(ui, tr!("destination-condition-range"));
                MenuFieldText::new(tr!("destination-condition-lower"), &mut draft.lower)
                    .hint_text(tr!("destination-condition-open"))
                    .show(ui);
                ui.checkbox(&mut draft.lower_inclusive, tr!("destination-condition-lower-inclusive"));
                MenuFieldText::new(tr!("destination-condition-upper"), &mut draft.upper)
                    .hint_text(tr!("destination-condition-open"))
                    .show(ui);
                ui.checkbox(&mut draft.upper_inclusive, tr!("destination-condition-upper-inclusive"));
            }
            let built = build_condition(draft, categorical);
            if let Err(message) = &built {
                ui.label(egui::RichText::new(message).color(ui.visuals().error_fg_color));
            }
            menu::menu_actions(ui, |ui| {
                let submitted = menu::dialog_confirm_pressed(ui.ctx());
                if (submitted || ui.add(MenuButton::new(tr!("common-apply")).primary().enabled(built.is_ok())).clicked()) && built.is_ok() {
                    apply = true;
                    close = true;
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                    close = true;
                }
            });
        });
    if apply
        && let Some(draft) = editor.schedule_condition_draft.as_ref()
        && let Ok(condition) = build_condition(draft, categorical)
    {
        let replacing = draft.replacing;
        let mut conditions = rule.conditions.clone();
        match replacing.and_then(|field| conditions.iter().position(|held| held.field == field)) {
            Some(position) => conditions[position] = condition,
            None => conditions.push(condition),
        }
        commands.push(UiCommand::schedule(session, ScheduleEdit::SetRuleConditions { rule: rule_id, conditions }));
    }
    if close || !open {
        editor.schedule_condition_draft = None;
    }
}

/// Turn the draft into a condition, or say what is wrong with it.
///
/// The same rules the domain applies, checked here so the dialog can refuse
/// before anything is committed rather than reporting after.
fn build_condition(draft: &ScheduleConditionDraft, categorical: bool) -> Result<FieldCondition, String> {
    let Some(field) = draft.field else {
        return Err(tr!("destination-condition-pick-field"));
    };
    let test = if categorical {
        if draft.values.is_empty() {
            return Err(crate::model::schedule::ScheduleError::EmptyCondition.message());
        }
        ConditionTest::Category { values: draft.values.clone() }
    } else {
        let bound = |text: &str, inclusive: bool| -> Result<Option<Bound>, String> {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                return Ok(None);
            }
            let Ok(value) = trimmed.parse::<f64>() else {
                return Err(tr!("schedule-rate-not-a-number"));
            };
            if !value.is_finite() {
                return Err(crate::model::schedule::ScheduleError::InvalidBound.message());
            }
            Ok(Some(Bound { value, inclusive }))
        };
        let lower = bound(&draft.lower, draft.lower_inclusive)?;
        let upper = bound(&draft.upper, draft.upper_inclusive)?;
        if lower.is_none() && upper.is_none() {
            return Err(crate::model::schedule::ScheduleError::EmptyCondition.message());
        }
        if let (Some(lower), Some(upper)) = (lower, upper) {
            let ordered = if lower.inclusive && upper.inclusive {
                lower.value <= upper.value
            } else {
                lower.value < upper.value
            };
            if !ordered {
                return Err(crate::model::schedule::ScheduleError::EmptyInterval.message());
            }
        }
        ConditionTest::Range { lower, upper }
    };
    Ok(FieldCondition { field, test })
}

/// The New Destination dialog. The kind comes from the page it was opened on,
/// so there is nothing to choose but a name.
pub(crate) fn draw_new_destination_dialog(ui: &mut egui::Ui, editor: &mut EditorState, plan: &SchedulePlan, session: u32, commands: &mut Vec<UiCommand>) {
    if !editor.new_destination_open {
        return;
    }
    let kind = editor.new_destination_kind;
    let taken: Vec<String> = plan.routing().standalone.iter().map(|entry| entry.name.clone()).collect();
    let error = name_problem(&editor.new_destination_name, taken.into_iter());
    let mut open = true;
    let mut close = false;
    let mut create = false;
    DragableMenu::new("new_destination_dialog", new_label(kind))
        .open(&mut open)
        .min_width(320.0)
        .show(ui.ctx(), |ui| {
            MenuFieldText::new(tr!("planning-name"), &mut editor.new_destination_name)
                .hint_text(tr!(literal = "Required"))
                .show(ui);
            menu::menu_note(ui, tr!("destination-type-fixed", kind = kind.label()));
            if let Some(message) = &error {
                ui.label(egui::RichText::new(message).color(ui.visuals().error_fg_color));
            }
            menu::menu_actions(ui, |ui| {
                let submitted = menu::dialog_confirm_pressed(ui.ctx());
                if (submitted || ui.add(MenuButton::new(tr!("common-create")).primary().enabled(error.is_none())).clicked()) && error.is_none() {
                    create = true;
                    close = true;
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                    close = true;
                }
            });
        });
    if create {
        commands.push(UiCommand::schedule(
            session,
            ScheduleEdit::AddDestination {
                name: editor.new_destination_name.trim().to_owned(),
                kind,
            },
        ));
    }
    if close || !open {
        editor.new_destination_open = false;
        editor.new_destination_name.clear();
    }
}

fn suggested_destination_name(plan: &SchedulePlan, document: &Document, kind: DestinationKind) -> String {
    let taken = destinations::available(document.solids(), plan.routing()).into_iter().map(|entry| entry.name);
    crate::model::schedule::suggested_name(&default_name(kind), taken)
}

fn default_name(kind: DestinationKind) -> String {
    match kind {
        DestinationKind::Stockpile => tr!("destination-default-stockpile"),
        DestinationKind::Dump => tr!("destination-default-dump"),
        DestinationKind::Crusher => tr!("destination-default-crusher"),
    }
}

fn new_label(kind: DestinationKind) -> String {
    match kind {
        DestinationKind::Stockpile => tr!("planning-new-stockpile"),
        DestinationKind::Dump => tr!("planning-new-dump"),
        DestinationKind::Crusher => tr!("destination-new-crusher"),
    }
}

fn empty_note(kind: DestinationKind) -> String {
    match kind {
        DestinationKind::Stockpile => tr!("destination-no-stockpiles"),
        DestinationKind::Dump => tr!("destination-no-dumps"),
        DestinationKind::Crusher => tr!("destination-no-crushers"),
    }
}

fn kind_page_title(kind: DestinationKind) -> String {
    match kind {
        DestinationKind::Stockpile => tr!("planning-stockpiles"),
        DestinationKind::Dump => tr!("planning-dumps"),
        DestinationKind::Crusher => tr!("destination-crushers"),
    }
}

fn list_id(kind: DestinationKind) -> &'static str {
    match kind {
        DestinationKind::Stockpile => "schedule_stockpile_list",
        DestinationKind::Dump => "schedule_dump_list",
        DestinationKind::Crusher => "schedule_crusher_list",
    }
}

/// The standalone half of a destination id, for the crusher lookups that only
/// apply to one. A solid-backed destination has no crusher calendar, and the
/// sentinel it produces matches nothing.
fn standalone_of(id: DestinationId) -> StandaloneDestinationId {
    match id {
        DestinationId::Standalone(id) => id,
        DestinationId::Solid(_) => StandaloneDestinationId(u64::MAX),
    }
}

fn tonnes(value: f64) -> String {
    tr_format!(literal = "%value% t", value = super::schedule_calendar::format_tonnes(value))
}

fn tonnes_per_day(value: f64) -> String {
    tr_format!(literal = "%value% t/day", value = super::schedule_calendar::format_tonnes(value))
}
