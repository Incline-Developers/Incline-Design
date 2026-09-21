//! The Schedule workspace's Cashflow page: what a moved tonne is worth.
//!
//! A compact rule table and a selected-rule editor, like the destination
//! rules - but with no move-up and move-down, because matching rules *add*
//! and order buys nothing. Two rules that describe the same movement are two
//! contributions, so a duplicate is a deliberate second payment rather than a
//! conflict to be resolved.
//!
//! What a rule is worth is signed and is never clamped here: a cost is a
//! negative value, and a movement expensive enough should be able to make
//! idling the better answer.

use super::schedule_destinations::{ConditionAction, condition_note, descendant_selected, draw_condition_dialog, open_condition_draft, scope_key, visible};
use crate::{
    i18n::{tr, tr_format},
    model::{
        Document,
        schedule::{ActivitySelection, DestinationSelection, LoaderAgentId, LoaderSelection, MovementSourceScope, MovementSourceSelection, SchedulePlan, cashflow, destinations},
    },
    ui::{
        EditorState,
        state::{ConditionOwner, ScheduleCashflowDraft, ScheduleEdit, UiCommand},
        widgets::{
            context_menu::{ChecklistRow, ContextMenuAction, Tick, checklist_popup, context_menu_popup, context_menu_separator},
            data_grid::{DataGrid, GridRow, PropertyTable, grid_checkbox_row, grid_choice_row, grid_named_row, grid_row, grid_select_row, grid_separator_row},
            explorer::explorer_note,
            menu,
        },
    },
};

/// The rule table: every cashflow rule, what it describes and what it pays.
pub(crate) fn draw_rule_list(ui: &mut egui::Ui, rect: egui::Rect, editor: &mut EditorState, plan: &SchedulePlan, document: &Document, session: u32, commands: &mut Vec<UiCommand>) {
    let rules = plan.cashflow();
    let currency = plan.currency();
    let mut selected = editor.schedule_selected_cashflow_rule;
    DataGrid::new("schedule_cashflow_list", rect, &tr!("cashflow"))
        .column_header(&tr!("planning-name"))
        .show(ui, |ui| {
            if rules.rules.is_empty() {
                explorer_note(ui, tr!("cashflow-no-rules"));
            }
            for rule in &rules.rules {
                let label = tr_format!(
                    literal = "%name% · %value%",
                    name = rule.name.clone(),
                    value = cashflow::format_value(rule.value_per_tonne, currency)
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
                let disabled = (!rule.enabled).then(|| tr!("cashflow-rule-disabled"));
                let response = grid_row(ui, GridRow::new(&label).error(disabled.as_deref()).selected(selected == Some(rule.id))).on_hover_text(tr_format!(
                    literal = "%label%\n%summary%",
                    label = label.clone(),
                    summary = summary
                ));
                if response.clicked() {
                    selected = Some(rule.id);
                }
                context_menu_popup(&response, &rule.name, |ui| {
                    if ContextMenuAction::new(tr!("cashflow-duplicate-rule")).show(ui).clicked() {
                        commands.push(UiCommand::schedule(session, ScheduleEdit::DuplicateCashflowRule(rule.id)));
                        ui.close();
                    }
                    if ContextMenuAction::new(tr!("cashflow-delete-rule")).show(ui).clicked() {
                        commands.push(UiCommand::schedule(session, ScheduleEdit::DeleteCashflowRule(rule.id)));
                        ui.close();
                    }
                });
            }
            let body = ui.available_rect_before_wrap();
            if body.is_positive() {
                let response = ui.interact(body, ui.id().with("new_cashflow_rule_space"), egui::Sense::click());
                context_menu_popup(&response, tr!("cashflow"), |ui| {
                    if ContextMenuAction::new(tr!("cashflow-new-rule")).show(ui).clicked() {
                        let name = crate::model::schedule::suggested_name(&tr!("cashflow-default-rule-name"), rules.rules.iter().map(|rule| rule.name.clone()));
                        commands.push(UiCommand::schedule(session, ScheduleEdit::AddCashflowRule { name }));
                        ui.close();
                    }
                });
            }
        });
    if editor.schedule_selected_cashflow_rule != selected {
        editor.schedule_cashflow_draft = None;
        editor.schedule_condition_draft = None;
    }
    editor.schedule_selected_cashflow_rule = selected;
}

/// The selected rule's editor: which movements it describes, and what it pays.
pub(crate) fn draw_rule_editor(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    editor: &mut EditorState,
    plan: &SchedulePlan,
    document: &Document,
    session: u32,
    commands: &mut Vec<UiCommand>,
) {
    let currency = plan.currency().to_owned();
    let Some(rule) = editor.schedule_selected_cashflow_rule.and_then(|id| plan.cashflow().rule(id)).cloned() else {
        draw_empty(ui, rect);
        return;
    };
    let source = (rule.name.clone(), rule.value_per_tonne.to_bits());
    if editor.schedule_cashflow_draft.as_ref().is_none_or(|draft| draft.id != rule.id || draft.source != source) {
        editor.schedule_cashflow_draft = Some(ScheduleCashflowDraft {
            id: rule.id,
            source,
            name: rule.name.clone(),
            // Full stored precision, not the rounded form the table shows: an
            // edit that changes nothing else must not round the value.
            value: rule.value_per_tonne.to_string(),
        });
    }
    let available = destinations::available(document.solids(), plan.routing());
    let stockpiles: Vec<_> = available
        .iter()
        .filter(|entry| entry.kind == crate::model::schedule::DestinationKind::Stockpile)
        .cloned()
        .collect();
    let sources = editor.schedule_routing_sources.clone();
    let categories = editor.schedule_category_values.clone();
    let mut collapsed = editor.schedule_source_collapsed.clone();
    let taken: Vec<String> = plan.cashflow().rules.iter().filter(|other| other.id != rule.id).map(|other| other.name.clone()).collect();
    let mut edits = Vec::new();
    let mut condition_action = None;
    let draft = editor.schedule_cashflow_draft.as_mut().expect("just ensured");
    let name_error = name_problem(&draft.name, taken.iter().cloned());
    let value_error = parse_value(&draft.value).err();
    DataGrid::new("schedule_cashflow_editor", rect, &rule.name)
        .column_header(&tr!("cashflow-rule"))
        .show(ui, |ui| {
            grid_separator_row(ui, &tr!("cashflow-rule"), 0);
            {
                let (cell, _) = grid_named_row(ui, &tr!("planning-name"), 0);
                if cell.is_positive() {
                    let response = ui.put(cell, egui::TextEdit::singleline(&mut draft.name).desired_width(cell.width()));
                    if let Some(message) = &name_error {
                        response.clone().on_hover_text(message);
                    }
                    if response.lost_focus() && name_error.is_none() && draft.name.trim() != rule.name {
                        edits.push(UiCommand::schedule(
                            session,
                            ScheduleEdit::RenameCashflowRule {
                                rule: rule.id,
                                name: draft.name.trim().to_owned(),
                            },
                        ));
                    }
                }
            }
            let mut enabled = rule.enabled;
            if grid_checkbox_row(ui, &tr!("cashflow-rule-enabled"), &mut enabled, 0) {
                edits.push(UiCommand::schedule(session, ScheduleEdit::SetCashflowRuleEnabled { rule: rule.id, enabled }));
            }
            {
                let label = tr_format!(literal = "%label% (%currency%/t)", label = tr!("cashflow-rule-value"), currency = currency.clone());
                let (cell, _) = grid_named_row(ui, &label, 0);
                if cell.is_positive() {
                    let response = ui.put(cell, egui::TextEdit::singleline(&mut draft.value).desired_width(cell.width()));
                    let note = value_error.clone().unwrap_or_else(|| tr!("cashflow-help"));
                    response.clone().on_hover_text(note);
                    if response.lost_focus()
                        && let Ok(value_per_tonne) = parse_value(&draft.value)
                        && value_per_tonne != rule.value_per_tonne
                    {
                        edits.push(UiCommand::schedule(session, ScheduleEdit::SetCashflowRuleValue { rule: rule.id, value_per_tonne }));
                    }
                }
            }

            grid_separator_row(ui, &tr!("cashflow-activity"), 0);
            {
                let mut activity = rule.activity;
                let options = [
                    ActivitySelection::All,
                    ActivitySelection::Only(crate::model::schedule::Activity::Dig),
                    ActivitySelection::Only(crate::model::schedule::Activity::Reclaim),
                ];
                if grid_choice_row(
                    ui,
                    ("cashflow_activity", rule.id.0),
                    &tr!("cashflow-activity"),
                    &mut activity,
                    options,
                    ActivitySelection::label,
                    0,
                ) && activity != rule.activity
                {
                    edits.push(UiCommand::schedule(session, ScheduleEdit::SetCashflowRuleActivity { rule: rule.id, activity }));
                }
            }

            grid_separator_row(ui, &tr!("cashflow-rule-loaders"), 0);
            {
                let all = matches!(rule.loaders, LoaderSelection::All);
                let held: Vec<LoaderAgentId> = match &rule.loaders {
                    LoaderSelection::All => Vec::new(),
                    LoaderSelection::Only(agents) => agents.clone(),
                };
                let summary = if all {
                    tr!("destination-rule-all-loaders")
                } else {
                    held.iter()
                        .map(|agent| plan.agent(*agent).map(|agent| agent.name.clone()).unwrap_or_else(|| tr!("schedule-error-unknown-agent")))
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                let response = grid_select_row(ui, ("cashflow_loaders", rule.id.0), &tr!("cashflow-rule-loaders"), &summary, 0);
                let mut next: Option<LoaderSelection> = None;
                checklist_popup(&response, tr!("cashflow-rule-loaders"), 240.0, |ui| {
                    if ChecklistRow::new(&tr!("destination-rule-all-loaders"), Tick::of(all, false)).show(ui).toggled {
                        next = Some(if all {
                            LoaderSelection::Only(plan.agents().iter().map(|agent| agent.id).collect())
                        } else {
                            LoaderSelection::All
                        });
                    }
                    context_menu_separator(ui);
                    if plan.agents().is_empty() {
                        menu::menu_note(ui, tr!("destination-rule-no-loaders"));
                    }
                    for agent in plan.agents() {
                        let picked = all || held.contains(&agent.id);
                        if ChecklistRow::new(&agent.name, Tick::of(picked, false)).depth(1).show(ui).toggled {
                            let mut list: Vec<LoaderAgentId> = if all { plan.agents().iter().map(|agent| agent.id).collect() } else { held.clone() };
                            if picked {
                                list.retain(|entry| *entry != agent.id);
                            } else {
                                list.push(agent.id);
                            }
                            next = (!list.is_empty()).then_some(LoaderSelection::Only(list));
                        }
                    }
                });
                if let Some(loaders) = next
                    && loaders != rule.loaders
                    && !matches!(&loaders, LoaderSelection::Only(agents) if agents.is_empty())
                {
                    edits.push(UiCommand::schedule(session, ScheduleEdit::SetCashflowRuleLoaders { rule: rule.id, loaders }));
                }
            }

            grid_separator_row(ui, &tr!("cashflow-rule-sources"), 0);
            {
                let all = matches!(rule.sources, MovementSourceSelection::All);
                let held: Vec<MovementSourceScope> = match &rule.sources {
                    MovementSourceSelection::All => Vec::new(),
                    MovementSourceSelection::Only(scopes) => scopes.clone(),
                };
                let summary = if all {
                    tr!("destination-rule-all-sources")
                } else {
                    held.iter()
                        .map(|scope| match scope {
                            MovementSourceScope::Ground(ground) => sources
                                .iter()
                                .find(|view| view.scope == *ground)
                                .map(|view| view.label.clone())
                                .unwrap_or_else(|| tr!("destination-source-unplaced")),
                            MovementSourceScope::Stockpile(id) => available
                                .iter()
                                .find(|entry| entry.id == *id)
                                .map(|entry| entry.name.clone())
                                .unwrap_or_else(|| tr!("destination-unresolved")),
                        })
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                let response = grid_select_row(ui, ("cashflow_sources", rule.id.0), &tr!("cashflow-rule-sources"), &summary, 0);
                let mut next: Option<MovementSourceSelection> = None;
                let everything = || -> Vec<MovementSourceScope> {
                    sources
                        .iter()
                        .filter(|view| view.depth == 0)
                        .map(|view| MovementSourceScope::Ground(view.scope))
                        .chain(stockpiles.iter().map(|entry| MovementSourceScope::Stockpile(entry.id)))
                        .collect()
                };
                let toggle = |held: &[MovementSourceScope], scope: MovementSourceScope, picked: bool| -> Option<MovementSourceSelection> {
                    let mut list: Vec<MovementSourceScope> = if held.is_empty() { everything() } else { held.to_vec() };
                    if picked {
                        list.retain(|entry| *entry != scope);
                    } else {
                        list.push(scope);
                    }
                    (!list.is_empty()).then_some(MovementSourceSelection::Only(list))
                };
                checklist_popup(&response, tr!("cashflow-rule-sources"), 260.0, |ui| {
                    if ChecklistRow::new(&tr!("destination-rule-all-sources"), Tick::of(all, false)).show(ui).toggled {
                        next = Some(if all {
                            match everything() {
                                scopes if scopes.is_empty() => MovementSourceSelection::All,
                                scopes => MovementSourceSelection::Only(scopes),
                            }
                        } else {
                            MovementSourceSelection::All
                        });
                    }
                    context_menu_separator(ui);
                    menu::menu_section(ui, tr!("truck-rule-ground"));
                    if sources.is_empty() {
                        menu::menu_note(ui, tr!("destination-no-sources"));
                    }
                    for (index, view) in sources.iter().enumerate() {
                        if !visible(&sources, index, &collapsed) {
                            continue;
                        }
                        let scope = MovementSourceScope::Ground(view.scope);
                        let children = sources.get(index + 1).is_some_and(|below| below.depth > view.depth);
                        let picked = all || held.contains(&scope);
                        let ground: Vec<_> = held
                            .iter()
                            .filter_map(|scope| match scope {
                                MovementSourceScope::Ground(ground) => Some(*ground),
                                MovementSourceScope::Stockpile(_) => None,
                            })
                            .collect();
                        let descendant = !picked && descendant_selected(&sources, index, &ground);
                        let acted = ChecklistRow::new(&view.label, Tick::of(picked, descendant))
                            .depth(view.depth)
                            .disclosure(children.then(|| !collapsed.contains(&scope_key(view.scope))))
                            .show(ui);
                        if acted.expanded {
                            let key = scope_key(view.scope);
                            match collapsed.iter().position(|held| *held == key) {
                                Some(position) => {
                                    collapsed.remove(position);
                                }
                                None => collapsed.push(key),
                            }
                        }
                        if acted.toggled {
                            next = toggle(if all { &[] } else { &held }, scope, picked);
                        }
                    }
                    context_menu_separator(ui);
                    menu::menu_section(ui, tr!("truck-rule-stockpiles"));
                    if stockpiles.is_empty() {
                        menu::menu_note(ui, tr!("destination-no-stockpiles"));
                    }
                    for entry in &stockpiles {
                        let scope = MovementSourceScope::Stockpile(entry.id);
                        let picked = all || held.contains(&scope);
                        if ChecklistRow::new(&entry.name, Tick::of(picked, false)).depth(1).show(ui).toggled {
                            next = toggle(if all { &[] } else { &held }, scope, picked);
                        }
                    }
                });
                if let Some(sources) = next
                    && sources != rule.sources
                {
                    edits.push(UiCommand::schedule(session, ScheduleEdit::SetCashflowRuleSources { rule: rule.id, sources }));
                }
            }

            grid_separator_row(ui, &tr!("cashflow-rule-destinations"), 0);
            {
                let all = matches!(rule.destinations, DestinationSelection::All);
                let held: Vec<_> = match &rule.destinations {
                    DestinationSelection::All => Vec::new(),
                    DestinationSelection::Only(ids) => ids.clone(),
                };
                let summary = if all {
                    tr!("cashflow-rule-all-destinations")
                } else {
                    held.iter()
                        .map(|id| {
                            available
                                .iter()
                                .find(|entry| entry.id == *id)
                                .map(|entry| entry.name.clone())
                                .unwrap_or_else(|| tr!("destination-unresolved"))
                        })
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                let response = grid_select_row(ui, ("cashflow_destinations", rule.id.0), &tr!("cashflow-rule-destinations"), &summary, 0);
                let mut next: Option<DestinationSelection> = None;
                checklist_popup(&response, tr!("cashflow-rule-destinations"), 260.0, |ui| {
                    if ChecklistRow::new(&tr!("cashflow-rule-all-destinations"), Tick::of(all, false)).show(ui).toggled {
                        next = Some(if all {
                            match available.iter().map(|entry| entry.id).collect::<Vec<_>>() {
                                ids if ids.is_empty() => DestinationSelection::All,
                                ids => DestinationSelection::Only(ids),
                            }
                        } else {
                            DestinationSelection::All
                        });
                    }
                    context_menu_separator(ui);
                    if available.is_empty() {
                        menu::menu_note(ui, tr!("destination-no-destinations"));
                    }
                    for entry in &available {
                        let picked = all || held.contains(&entry.id);
                        let label = tr_format!(literal = "%name% · %kind%", name = entry.name.clone(), kind = entry.kind.label());
                        if ChecklistRow::new(&label, Tick::of(picked, false)).depth(1).show(ui).toggled {
                            let mut list = if all {
                                available.iter().map(|entry| entry.id).collect::<Vec<_>>()
                            } else {
                                held.clone()
                            };
                            if picked {
                                list.retain(|entry_id| *entry_id != entry.id);
                            } else {
                                list.push(entry.id);
                            }
                            next = (!list.is_empty()).then_some(DestinationSelection::Only(list));
                        }
                    }
                });
                if let Some(destinations) = next
                    && destinations != rule.destinations
                {
                    edits.push(UiCommand::schedule(session, ScheduleEdit::SetCashflowRuleDestinations { rule: rule.id, destinations }));
                }
            }

            // Conditions, ANDed, read against the same contributing material
            // the destination rules match on.
            grid_separator_row(ui, &tr!("cashflow-rule-conditions"), 0);
            if rule.conditions.is_empty() {
                explorer_note(ui, tr!("destination-no-conditions"));
            }
            // The same policy, from the same helper, as the destination rules.
            super::schedule_destinations::category_conflict_note(ui, document, &rule.sources, &rule.conditions);
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
                let response = ui.interact(body, ui.id().with(("new_cashflow_condition_space", rule.id.0)), egui::Sense::click());
                context_menu_popup(&response, tr!("cashflow-rule-conditions"), |ui| {
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
    editor.schedule_source_collapsed = collapsed;
    if let Some(action) = condition_action {
        if let ConditionAction::Delete(field) = action {
            let conditions = rule.conditions.iter().filter(|condition| condition.field != field).cloned().collect();
            edits.push(UiCommand::schedule(session, ScheduleEdit::SetCashflowRuleConditions { rule: rule.id, conditions }));
        }
        open_condition_draft(editor, document, ConditionOwner::Cashflow(rule.id), &rule.conditions, action);
    }
    commands.append(&mut edits);
    draw_condition_dialog(ui, editor, plan, document, &categories, session, commands);
}

/// Nothing selected: the page says which half of it to click.
fn draw_empty(ui: &mut egui::Ui, rect: egui::Rect) {
    PropertyTable::new("schedule_cashflow_empty", rect, &tr!("cashflow")).show(ui, |rows| {
        rows.header(&tr!("planning-property"), &tr!("planning-value"));
        rows.readonly(&tr!("cashflow-rule"), &tr!("cashflow-select-rule"), None, None)
            .on_hover_text(tr!("cashflow-help"));
    });
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

/// Parse a typed value per tonne. Signed, and zero is a real answer; only
/// something that is not a finite number is refused.
fn parse_value(text: &str) -> Result<f64, String> {
    let trimmed = text.trim();
    let Ok(value) = trimmed.parse::<f64>() else {
        return Err(tr!("schedule-rate-not-a-number"));
    };
    if !value.is_finite() {
        return Err(crate::model::schedule::ScheduleError::InvalidValue.message());
    }
    Ok(value)
}
