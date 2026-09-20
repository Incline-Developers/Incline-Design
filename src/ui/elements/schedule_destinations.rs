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
        schedule::{
            Bound, ConditionTest, DestinationId, DestinationKind, FieldCondition, LoaderAgentId, LoaderSelection, MovementSourceScope, MovementSourceSelection, SchedulePlan,
            SourceScope, StandaloneDestinationId, destinations,
        },
    },
    ui::{
        EditorState,
        state::{ConditionOwner, ScheduleConditionDraft, ScheduleDestinationDraft, ScheduleEdit, ScheduleRuleDraft, SourceScopeView, UiCommand},
        widgets::{
            context_menu::{ChecklistRow, ContextMenuAction, Tick, checklist_popup, context_menu_popup, context_menu_separator},
            data_grid::{DataGrid, GridRow, PropertyTable, grid_named_row, grid_row, grid_select_row, grid_separator_row, property_table_height},
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
) -> egui::Rect {
    let selected = editor
        .schedule_selected_destination
        .and_then(|id| destinations::resolve(id, document.solids(), plan.routing()).ok())
        .filter(|entry| entry.kind == kind);
    let Some(entry) = selected else {
        let table_rect = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), property_table_height(ui, 2).min(rect.height())));
        PropertyTable::new("schedule_destination_properties", table_rect, &kind_page_title(kind)).show(ui, |rows| {
            rows.header(&tr!("planning-property"), &tr!("planning-value"));
            rows.readonly(&tr!("planning-name"), &tr!("destination-select"), None, None);
        });
        return table_rect;
    };
    let crusher_default = plan
        .routing()
        .crusher(entry.solid.map_or_else(|| standalone_of(entry.id), |_| standalone_of(entry.id)))
        .and_then(|calendar| calendar.default_tpd);
    let crusher_default = if entry.kind == DestinationKind::Crusher { crusher_default } else { None };
    let source = (entry.name.clone(), entry.capacity_t, crusher_default, entry.distance_km.to_bits());
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
            distance: super::schedule_trucking::number(entry.distance_km),
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
    let distance_error = super::schedule_trucking::parse_positive(&draft.distance, crate::model::schedule::ScheduleError::InvalidDistance).err();
    // Two extra rows on a stockpile: which end reclaim takes from, and what it
    // opens holding. The opening total is calculated from the lots beside it and
    // is never typed here - one figure, one place it comes from.
    let stockpile = entry.kind == DestinationKind::Stockpile;
    let rows_used = 5 + usize::from(linked) + 2 * usize::from(stockpile);
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
        if stockpile {
            let mut order = entry.reclaim_order;
            let selected = order.label();
            let response = rows.combo(
                ("reclaim_order", format!("{:?}", entry.id)),
                &tr!("inventory-reclaim-order"),
                &mut order,
                &selected,
                [
                    (crate::model::schedule::ReclaimOrder::Fifo, crate::model::schedule::ReclaimOrder::Fifo.label()),
                    (crate::model::schedule::ReclaimOrder::Lifo, crate::model::schedule::ReclaimOrder::Lifo.label()),
                ],
            );
            response.on_hover_text(tr!("inventory-help"));
            if order != entry.reclaim_order {
                edits.push(UiCommand::schedule(session, ScheduleEdit::SetReclaimOrder { destination: entry.id, order }));
            }
            let over = entry
                .capacity_t
                .is_some_and(|capacity| entry.opening_t > capacity)
                .then(|| crate::model::schedule::ScheduleError::OpeningOverCapacity.message());
            rows.readonly(&tr!("inventory-opening-tonnes"), &tonnes(entry.opening_t), None, over.as_deref());
        }
        // A haul distance, not a measurement: it is one number for every source
        // that delivers here, and nothing derives it from where the solid sits.
        let response = rows.field(&tr!("destination-distance"), &mut draft.distance, distance_error.as_deref());
        if response.lost_focus()
            && let Ok(distance_km) = super::schedule_trucking::parse_positive(&draft.distance, crate::model::schedule::ScheduleError::InvalidDistance)
            && distance_km != entry.distance_km
        {
            edits.push(UiCommand::schedule(
                session,
                ScheduleEdit::SetDestinationDistance {
                    destination: entry.id,
                    distance_km,
                },
            ));
        }
    });
    commands.append(&mut edits);
    table_rect
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
                // Every destination the rule allows, in the order it tries them.
                let target = rule
                    .destinations
                    .iter()
                    .map(|destination| {
                        destinations::resolve(*destination, document.solids(), routing)
                            .map(|entry| entry.name)
                            .unwrap_or_else(|_| tr!("destination-unresolved"))
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
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
        commands.push(UiCommand::schedule(
            session,
            ScheduleEdit::AddRule {
                name,
                destinations: vec![first.id],
            },
        ));
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
    let stockpiles: Vec<_> = available.iter().filter(|entry| entry.kind == DestinationKind::Stockpile).cloned().collect();
    let source_views = editor.schedule_routing_sources.clone();
    let categories = editor.schedule_category_values.clone();
    let mut edits = Vec::new();
    let taken: Vec<String> = routing.rules.iter().filter(|other| other.id != rule.id).map(|other| other.name.clone()).collect();
    let draft = editor.schedule_rule_draft.as_mut().expect("just ensured");
    let name_error = name_problem(&draft.name, taken.iter().cloned());
    let mut condition_action = None;
    // Cloned for the frame: the grid holds the draft mutably while it draws, and
    // the folded set is small enough that a clone is cheaper than splitting the
    // editor borrow.
    let mut collapsed = editor.schedule_source_collapsed.clone();
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
            // Destinations, tried in the order they are listed. A rule may allow
            // several: "this material may go to ROM A or ROM B" is one decision,
            // and writing it as two identical rules would make it two.
            {
                let names: Vec<String> = rule
                    .destinations
                    .iter()
                    .map(|id| {
                        available
                            .iter()
                            .find(|entry| entry.id == *id)
                            .map(|entry| entry.name.clone())
                            .unwrap_or_else(|| tr!("destination-unresolved"))
                    })
                    .collect();
                let summary = if names.is_empty() { tr!("destination-rule-none") } else { names.join(", ") };
                let response = grid_select_row(ui, ("rule_destinations", rule.id.0), &tr!("destination-rule-target"), &summary, 0);
                let mut chosen = rule.destinations.clone();
                checklist_popup(&response, tr!("destination-rule-target"), 260.0, |ui| {
                    if available.is_empty() {
                        menu::menu_note(ui, tr!("destination-no-destinations"));
                        return;
                    }
                    let all = available.iter().all(|entry| chosen.contains(&entry.id));
                    let any = available.iter().any(|entry| chosen.contains(&entry.id));
                    if ChecklistRow::new(&tr!("destination-select-all"), Tick::of(all, any)).show(ui).toggled && !all {
                        chosen = available.iter().map(|entry| entry.id).collect();
                    }
                    context_menu_separator(ui);
                    for entry in &available {
                        let position = chosen.iter().position(|held| *held == entry.id);
                        // Numbered while selected, because the order they are
                        // listed in is the order they are tried.
                        let label = match position {
                            Some(position) => tr_format!(
                                literal = "%order%. %name% · %kind%",
                                order = (position + 1).to_string(),
                                name = entry.name.clone(),
                                kind = entry.kind.label()
                            ),
                            None => tr_format!(literal = "%name% · %kind%", name = entry.name.clone(), kind = entry.kind.label()),
                        };
                        if ChecklistRow::new(&label, Tick::of(position.is_some(), false)).depth(1).show(ui).toggled {
                            match position {
                                Some(position) => {
                                    chosen.remove(position);
                                }
                                None => chosen.push(entry.id),
                            }
                        }
                    }
                });
                // An empty list is not a state a rule can hold - it would match
                // material and have nowhere to put it - so the last tick stays
                // until another is put in its place.
                if !chosen.is_empty() && chosen != rule.destinations {
                    edits.push(UiCommand::schedule(
                        session,
                        ScheduleEdit::SetRuleDestinations {
                            rule: rule.id,
                            destinations: chosen,
                        },
                    ));
                }
            }

            // Loaders. "All" is not the same as "every loader currently in the
            // fleet": a machine added tomorrow is covered by All and is not added
            // to a list of names. Turning All off writes out the list it stands
            // for today, which is where editing from it starts.
            grid_separator_row(ui, &tr!("destination-rule-loaders"), 0);
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
                let response = grid_select_row(ui, ("rule_loaders", rule.id.0), &tr!("destination-rule-loaders"), &summary, 0);
                let mut next: Option<LoaderSelection> = None;
                checklist_popup(&response, tr!("destination-rule-loaders"), 240.0, |ui| {
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
                            // The last tick stays until All is chosen. An empty
                            // list is not "everything": it is a restriction that
                            // matches nothing, and reading it as All would widen
                            // the rule at the moment the user narrowed it most.
                            next = (!list.is_empty()).then_some(LoaderSelection::Only(list));
                        }
                    }
                });
                if let Some(loaders) = next
                    && loaders != rule.loaders
                    && !matches!(&loaders, LoaderSelection::Only(agents) if agents.is_empty())
                {
                    edits.push(UiCommand::schedule(session, ScheduleEdit::SetRuleLoaders { rule: rule.id, loaders }));
                }
            }

            // Sources: the bands the last completed Solids run produced, nested
            // pit → bench → flitch so a whole area can be taken in one tick or
            // folded away, and the stockpiles a reclaim would load from. One
            // widget, shared with the trucking and cashflow rules, so what the
            // three pages call a source cannot drift apart.
            grid_separator_row(ui, &tr!("destination-rule-sources"), 0);
            if let Some(sources) = movement_sources_row(
                ui,
                ("rule_sources", rule.id.0),
                &tr!("destination-rule-sources"),
                &rule.sources,
                &source_views,
                &stockpiles,
                &mut collapsed,
            ) && sources != rule.sources
            {
                edits.push(UiCommand::schedule(session, ScheduleEdit::SetRuleSources { rule: rule.id, sources }));
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
    editor.schedule_source_collapsed = collapsed;
    match condition_action {
        None => {}
        Some(ConditionAction::Add) => {
            editor.schedule_condition_draft = Some(ScheduleConditionDraft {
                rule: ConditionOwner::Routing(rule.id),
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
                rule: ConditionOwner::Routing(rule.id),
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

pub(crate) enum ConditionAction {
    Add,
    Edit(crate::model::ReserveFieldId),
    Delete(crate::model::ReserveFieldId),
}

/// Seed the condition dialog for one rule, whichever kind it is.
///
/// Shared so the two rule editors open the same dialog on the same draft
/// rather than each building one.
pub(crate) fn open_condition_draft(editor: &mut EditorState, document: &Document, owner: ConditionOwner, conditions: &[FieldCondition], action: ConditionAction) {
    match action {
        ConditionAction::Delete(_) => {}
        ConditionAction::Add => {
            editor.schedule_condition_draft = Some(ScheduleConditionDraft {
                rule: owner,
                replacing: None,
                field: document
                    .reserve_fields()
                    .iter()
                    .find(|field| !conditions.iter().any(|condition| condition.field == field.id))
                    .map(|field| field.id),
                values: Vec::new(),
                lower: String::new(),
                lower_inclusive: false,
                upper: String::new(),
                upper_inclusive: false,
            });
        }
        ConditionAction::Edit(field) => {
            let condition = conditions.iter().find(|condition| condition.field == field);
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
                rule: owner,
                replacing: Some(field),
                field: Some(field),
                values,
                lower,
                lower_inclusive,
                upper,
                upper_inclusive,
            });
        }
    }
}

/// What a condition is read against, stated on demand rather than on the page.
///
/// A summed field's condition compares the *contributing row's own* mapped
/// value, not the tonnes that row contributed - which is the one thing about
/// this that is not obvious from the expression.
pub(crate) fn condition_note(document: &Document, condition: &FieldCondition) -> String {
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
pub(crate) fn draw_condition_dialog(
    ui: &mut egui::Ui,
    editor: &mut EditorState,
    plan: &SchedulePlan,
    document: &Document,
    categories: &std::collections::BTreeMap<crate::model::ReserveFieldId, Vec<String>>,
    session: u32,
    commands: &mut Vec<UiCommand>,
) {
    let Some(draft) = editor.schedule_condition_draft.as_mut() else { return };
    let owner = draft.rule;
    // Whichever kind of rule it belongs to, a condition is the same statement
    // about the same field variable, so one dialog writes both.
    let held: Option<Vec<FieldCondition>> = match owner {
        ConditionOwner::Routing(id) => plan.routing().rule(id).map(|rule| rule.conditions.clone()),
        ConditionOwner::Cashflow(id) => plan.cashflow().rule(id).map(|rule| rule.conditions.clone()),
    };
    let Some(existing) = held else {
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
                .filter(|entry| !existing.iter().any(|condition| condition.field == entry.id) || draft.replacing == Some(entry.id))
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
        let mut conditions = existing.clone();
        match replacing.and_then(|field| conditions.iter().position(|held| held.field == field)) {
            Some(position) => conditions[position] = condition,
            None => conditions.push(condition),
        }
        commands.push(UiCommand::schedule(
            session,
            match owner {
                ConditionOwner::Routing(rule) => ScheduleEdit::SetRuleConditions { rule, conditions },
                ConditionOwner::Cashflow(rule) => ScheduleEdit::SetCashflowRuleConditions { rule, conditions },
            },
        ));
    }
    if close || !open {
        editor.schedule_condition_draft = None;
    }
}

/// Turn the draft into a condition, or say what is wrong with it.
///
/// The same rules the domain applies, checked here so the dialog can refuse
/// before anything is committed rather than reporting after.
pub(crate) fn build_condition(draft: &ScheduleConditionDraft, categorical: bool) -> Result<FieldCondition, String> {
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

/// Whether a source row is shown: every group above it is open.
///
/// Ancestry is read off the depths, which is how the flat list the run produces
/// describes its nesting - the nearest row above with a smaller depth is the
/// parent.
/// The one movement-source selector, shared by the destination, trucking and
/// cashflow rules.
///
/// Ground nested pit → bench → flitch, then the stockpiles a reclaim would load
/// from, then whatever the rule holds that the current run cannot place - kept
/// and named rather than dropped, because a rule that quietly stopped
/// restricting would move material somewhere nobody chose.
///
/// `All` is explicit throughout. Turning it off writes out the list it stands
/// for today, which is where editing from it starts; unticking the last entry
/// does nothing, because an empty list matches nothing and reading it as All
/// would widen the rule at the moment the user narrowed it most. Returns the
/// new selection, or `None` when nothing was chosen this frame.
pub(crate) fn movement_sources_row(
    ui: &mut egui::Ui,
    id: (&'static str, u64),
    label: &str,
    held: &MovementSourceSelection,
    sources: &[SourceScopeView],
    stockpiles: &[destinations::DestinationView],
    collapsed: &mut Vec<(u64, u64, u64)>,
) -> Option<MovementSourceSelection> {
    let all = matches!(held, MovementSourceSelection::All);
    let chosen: Vec<MovementSourceScope> = match held {
        MovementSourceSelection::All => Vec::new(),
        MovementSourceSelection::Only(scopes) => scopes.clone(),
    };
    let summary = if all {
        tr!("destination-rule-all-sources")
    } else {
        chosen
            .iter()
            .map(|scope| match scope {
                MovementSourceScope::Ground(ground) => sources
                    .iter()
                    .find(|view| view.scope == *ground)
                    .map(|view| view.label.clone())
                    .unwrap_or_else(|| tr!("destination-source-unplaced")),
                MovementSourceScope::Stockpile(stockpile) => stockpiles
                    .iter()
                    .find(|entry| entry.id == *stockpile)
                    .map(|entry| entry.name.clone())
                    .unwrap_or_else(|| tr!("destination-unresolved")),
            })
            .collect::<Vec<_>>()
            .join(", ")
    };
    // The pits and the piles, not every band of them: a whole pit is what All
    // stood for, said as scopes.
    let everything = || -> Vec<MovementSourceScope> {
        sources
            .iter()
            .filter(|view| view.depth == 0)
            .map(|view| MovementSourceScope::Ground(view.scope))
            .chain(stockpiles.iter().map(|entry| MovementSourceScope::Stockpile(entry.id)))
            .collect()
    };
    let toggle = |scope: MovementSourceScope, picked: bool| -> Option<MovementSourceSelection> {
        let mut list = if all { everything() } else { chosen.clone() };
        if picked {
            list.retain(|entry| *entry != scope);
        } else {
            list.push(scope);
        }
        (!list.is_empty()).then_some(MovementSourceSelection::Only(list))
    };
    let response = grid_select_row(ui, id, label, &summary, 0);
    let mut next: Option<MovementSourceSelection> = None;
    checklist_popup(&response, label.to_owned(), 260.0, |ui| {
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
        let ground: Vec<SourceScope> = chosen.iter().filter_map(|scope| scope.ground()).collect();
        for (index, view) in sources.iter().enumerate() {
            if !visible(sources, index, collapsed) {
                continue;
            }
            let scope = MovementSourceScope::Ground(view.scope);
            let children = sources.get(index + 1).is_some_and(|below| below.depth > view.depth);
            let picked = all || chosen.contains(&scope);
            let descendant = !picked && descendant_selected(sources, index, &ground);
            let acted = ChecklistRow::new(&view.label, Tick::of(picked, descendant))
                .depth(view.depth)
                .disclosure(children.then(|| !collapsed.contains(&scope_key(view.scope))))
                .show(ui);
            if acted.expanded {
                let key = scope_key(view.scope);
                match collapsed.iter().position(|folded| *folded == key) {
                    Some(position) => {
                        collapsed.remove(position);
                    }
                    None => collapsed.push(key),
                }
            }
            if acted.toggled {
                next = toggle(scope, picked);
            }
        }
        context_menu_separator(ui);
        menu::menu_section(ui, tr!("truck-rule-stockpiles"));
        if stockpiles.is_empty() {
            menu::menu_note(ui, tr!("destination-no-stockpiles"));
        }
        for entry in stockpiles {
            let scope = MovementSourceScope::Stockpile(entry.id);
            let picked = all || chosen.contains(&scope);
            if ChecklistRow::new(&entry.name, Tick::of(picked, false)).depth(1).show(ui).toggled {
                next = toggle(scope, picked);
            }
        }
        // What this rule holds that the run cannot place, or that names a
        // destination the project no longer has.
        let unresolved: Vec<MovementSourceScope> = chosen
            .iter()
            .copied()
            .filter(|scope| match scope {
                MovementSourceScope::Ground(ground) => !sources.iter().any(|view| view.scope == *ground),
                MovementSourceScope::Stockpile(id) => !stockpiles.iter().any(|entry| entry.id == *id),
            })
            .collect();
        if !unresolved.is_empty() {
            context_menu_separator(ui);
        }
        for scope in unresolved {
            let label = match scope {
                MovementSourceScope::Ground(_) => tr!("destination-source-unplaced"),
                MovementSourceScope::Stockpile(_) => tr!("destination-unresolved"),
            };
            if ChecklistRow::new(&label, Tick::On).depth(1).show(ui).toggled {
                next = toggle(scope, true);
            }
        }
    });
    next
}

pub(crate) fn visible(sources: &[SourceScopeView], index: usize, collapsed: &[(u64, u64, u64)]) -> bool {
    let mut depth = sources[index].depth;
    for above in sources[..index].iter().rev() {
        if above.depth < depth {
            if collapsed.contains(&scope_key(above.scope)) {
                return false;
            }
            depth = above.depth;
            if depth == 0 {
                break;
            }
        }
    }
    true
}

/// Whether anything nested under this row is selected, which is what puts a
/// folded group's box in its mixed state rather than leaving it empty.
pub(crate) fn descendant_selected(sources: &[SourceScopeView], index: usize, held: &[SourceScope]) -> bool {
    let depth = sources[index].depth;
    sources[index + 1..].iter().take_while(|below| below.depth > depth).any(|below| held.contains(&below.scope))
}

/// A folded group's key. A pit carries no band, and a band's base is always
/// below its top, so the two can never collide.
pub(crate) fn scope_key(scope: SourceScope) -> (u64, u64, u64) {
    let bits = |value: f64| if value == 0.0 { 0.0_f64.to_bits() } else { value.to_bits() };
    match scope {
        SourceScope::Pit(solid) => (solid.0, 0, 0),
        SourceScope::Bench { solid, base, top } | SourceScope::Flitch { solid, base, top } => (solid.0, bits(base), bits(top)),
    }
}

/// The opening-inventory list for the selected stockpile: its lots, oldest
/// first.
///
/// Ordered, and the order is labelled, because FIFO reads it from the top and
/// LIFO from the bottom - so which end a lot sits at is the whole difference
/// between the two settings.
pub(crate) fn draw_opening_lots(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    editor: &mut EditorState,
    plan: &SchedulePlan,
    document: &Document,
    session: u32,
    commands: &mut Vec<UiCommand>,
) {
    let selected_destination = editor
        .schedule_selected_destination
        .and_then(|id| destinations::resolve(id, document.solids(), plan.routing()).ok())
        .filter(|entry| entry.kind == DestinationKind::Stockpile);
    let Some(destination) = selected_destination else {
        DataGrid::new("schedule_opening_lots", rect, &tr!("inventory-opening")).show(ui, |ui| {
            explorer_note(ui, tr!("destination-select"));
        });
        return;
    };
    let inventory = plan.routing().inventory(destination.id).cloned().unwrap_or_default();
    let mut selected = editor.schedule_selected_lot;
    DataGrid::new("schedule_opening_lots", rect, &tr!("inventory-opening"))
        .column_header(&tr!("inventory-opening-order-note"))
        .show(ui, |ui| {
            if inventory.lots.is_empty() {
                explorer_note(ui, tr!("inventory-no-lots"));
            }
            let last = inventory.lots.len().saturating_sub(1);
            for (position, lot) in inventory.lots.iter().enumerate() {
                let label = tr_format!(literal = "%name% · %tonnes%", name = lot.name.clone(), tonnes = tonnes(lot.tonnes()));
                let response = grid_row(ui, GridRow::new(&label).selected(selected == Some(lot.id))).on_hover_text(&label);
                if response.clicked() {
                    selected = Some(lot.id);
                }
                context_menu_popup(&response, &lot.name, |ui| {
                    if ContextMenuAction::new(tr!("inventory-duplicate-lot")).show(ui).clicked() {
                        commands.push(UiCommand::schedule(
                            session,
                            ScheduleEdit::DuplicateOpeningLot {
                                destination: destination.id,
                                lot: lot.id,
                            },
                        ));
                        ui.close();
                    }
                    if ContextMenuAction::new(tr!("inventory-delete-lot")).show(ui).clicked() {
                        commands.push(UiCommand::schedule(
                            session,
                            ScheduleEdit::DeleteOpeningLot {
                                destination: destination.id,
                                lot: lot.id,
                            },
                        ));
                        ui.close();
                    }
                    context_menu_separator(ui);
                    if ContextMenuAction::new(tr!("inventory-move-lot-older")).enabled(position > 0).show(ui).clicked() {
                        commands.push(UiCommand::schedule(
                            session,
                            ScheduleEdit::MoveOpeningLot {
                                destination: destination.id,
                                lot: lot.id,
                                newer: false,
                            },
                        ));
                        ui.close();
                    }
                    if ContextMenuAction::new(tr!("inventory-move-lot-newer")).enabled(position < last).show(ui).clicked() {
                        commands.push(UiCommand::schedule(
                            session,
                            ScheduleEdit::MoveOpeningLot {
                                destination: destination.id,
                                lot: lot.id,
                                newer: true,
                            },
                        ));
                        ui.close();
                    }
                });
            }
            let body = ui.available_rect_before_wrap();
            if body.is_positive() {
                let response = ui.interact(body, ui.id().with("new_opening_lot_space"), egui::Sense::click());
                context_menu_popup(&response, tr!("inventory-opening"), |ui| {
                    if ContextMenuAction::new(tr!("inventory-new-lot")).show(ui).clicked() {
                        let name = crate::model::schedule::suggested_name(&tr!("inventory-default-lot-name"), inventory.lots.iter().map(|lot| lot.name.clone()));
                        commands.push(UiCommand::schedule(
                            session,
                            ScheduleEdit::AddOpeningLot {
                                destination: destination.id,
                                name,
                                tonnes_t: DEFAULT_LOT_TONNES,
                            },
                        ));
                        ui.close();
                    }
                });
            }
        });
    if editor.schedule_selected_lot != selected {
        editor.schedule_lot_draft = None;
    }
    editor.schedule_selected_lot = selected;
}

/// What a new lot is created holding, so it is stock rather than an empty row
/// the user has to fill in before it means anything.
const DEFAULT_LOT_TONNES: f64 = 1000.0;

/// The selected lot: its name, and each portion's tonnes and property values.
///
/// A portion's tonnes are asked once, here. The nominated tonnage field is not
/// offered as a property - it would be the same number entered twice, free to
/// disagree with itself.
pub(crate) fn draw_lot_editor(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    editor: &mut EditorState,
    plan: &SchedulePlan,
    document: &Document,
    session: u32,
    commands: &mut Vec<UiCommand>,
) {
    let selected_destination = editor
        .schedule_selected_destination
        .and_then(|id| destinations::resolve(id, document.solids(), plan.routing()).ok())
        .filter(|entry| entry.kind == DestinationKind::Stockpile);
    let Some(destination) = selected_destination else { return };
    let inventory = plan.routing().inventory(destination.id).cloned().unwrap_or_default();
    let Some(lot) = editor.schedule_selected_lot.and_then(|id| inventory.lot(id)).cloned() else {
        DataGrid::new("schedule_lot_editor", rect, &tr!("inventory-lot")).show(ui, |ui| {
            explorer_note(ui, tr!("inventory-select-lot"));
        });
        return;
    };
    // The tonnage field is supplied by the portion's own tonnes, so it is not
    // offered as a property to enter a second time.
    let tonnage_field = plan.tonnage_field();
    let mut fields: Vec<_> = document
        .reserve_fields()
        .iter()
        .filter(|field| Some(field.id) != tonnage_field)
        .map(|field| (field.id, field.name.clone(), field.aggregation == ReserveAggregation::Category))
        .collect();
    for (field, value) in lot.portions.iter().flat_map(|portion| &portion.values) {
        if Some(*field) != tonnage_field && !fields.iter().any(|(id, _, _)| id == field) {
            fields.push((
                *field,
                tr!("inventory-field-missing", id = field.0.to_string()),
                matches!(value, crate::model::schedule::OpeningValue::Category(_)),
            ));
        }
    }
    let categories = editor.schedule_category_values.clone();
    let source = format!("{lot:?}");
    if editor
        .schedule_lot_draft
        .as_ref()
        .is_none_or(|draft| draft.destination != destination.id || draft.lot != lot.id || draft.source != source)
    {
        editor.schedule_lot_draft = Some(crate::ui::state::ScheduleLotDraft {
            destination: destination.id,
            lot: lot.id,
            source,
            name: lot.name.clone(),
            portions: lot.portions.iter().map(|portion| (portion.id, portion.tonnes_t.to_string())).collect(),
            values: lot
                .portions
                .iter()
                .flat_map(|portion| {
                    portion.values.iter().filter_map(move |(field, value)| match value {
                        crate::model::schedule::OpeningValue::Number(number) => Some((portion.id, *field, number.to_string())),
                        crate::model::schedule::OpeningValue::Category(_) => None,
                    })
                })
                .collect(),
        });
    }
    let taken: Vec<String> = inventory.lots.iter().filter(|other| other.id != lot.id).map(|other| other.name.clone()).collect();
    let draft = editor.schedule_lot_draft.as_mut().expect("just ensured");
    let name_error = name_problem(&draft.name, taken.iter().cloned());
    let mut edits = Vec::new();
    let mut portion_action = None;
    DataGrid::new("schedule_lot_editor", rect, &lot.name).column_header(&tr!("inventory-lot")).show(ui, |ui| {
        grid_separator_row(ui, &tr!("inventory-lot"), 0);
        {
            let (cell, _) = grid_named_row(ui, &tr!("inventory-lot-name"), 0);
            if cell.is_positive() {
                let response = ui.put(cell, egui::TextEdit::singleline(&mut draft.name).desired_width(cell.width()));
                if let Some(message) = &name_error {
                    response.clone().on_hover_text(message);
                }
                if response.lost_focus() && name_error.is_none() && draft.name.trim() != lot.name {
                    edits.push(UiCommand::schedule(
                        session,
                        ScheduleEdit::RenameOpeningLot {
                            destination: destination.id,
                            lot: lot.id,
                            name: draft.name.trim().to_owned(),
                        },
                    ));
                }
            }
        }
        let (cell, _) = grid_named_row(ui, &tr!("inventory-lot-tonnes"), 0);
        if cell.is_positive() {
            ui.put(
                cell,
                egui::Label::new(egui::RichText::new(tonnes(lot.tonnes())).color(ui.visuals().weak_text_color()))
                    .truncate()
                    .halign(egui::Align::Min),
            );
        }

        for (index, portion) in lot.portions.iter().enumerate() {
            let title = tr_format!(literal = "%portion% %index%", portion = tr!("inventory-portion"), index = (index + 1).to_string());
            grid_separator_row(ui, &title, 0);
            {
                let (cell, response) = grid_named_row(ui, &tr!("inventory-lot-tonnes"), 1);
                context_menu_popup(&response, &title, |ui| {
                    if ContextMenuAction::new(tr!("inventory-new-portion")).show(ui).clicked() {
                        portion_action = Some(PortionAction::Add);
                        ui.close();
                    }
                    // The last portion is refused rather than hidden: a lot
                    // with nothing in it is not stock, and deleting the lot
                    // is the edit that says so.
                    if ContextMenuAction::new(tr!("inventory-delete-portion")).enabled(lot.portions.len() > 1).show(ui).clicked() {
                        portion_action = Some(PortionAction::Delete(portion.id));
                        ui.close();
                    }
                });
                if cell.is_positive() {
                    let text = draft
                        .portions
                        .iter_mut()
                        .find(|(id, _)| *id == portion.id)
                        .map(|(_, text)| text)
                        .expect("the draft holds every portion");
                    let error = parse_lot_tonnes(text).err();
                    let response = ui.put(cell, egui::TextEdit::singleline(text).desired_width(cell.width()));
                    if let Some(message) = &error {
                        response.clone().on_hover_text(message);
                    }
                    if response.lost_focus()
                        && let Ok(tonnes_t) = parse_lot_tonnes(text)
                        && tonnes_t != portion.tonnes_t
                    {
                        edits.push(UiCommand::schedule(
                            session,
                            ScheduleEdit::SetOpeningPortionTonnes {
                                destination: destination.id,
                                lot: lot.id,
                                portion: portion.id,
                                tonnes_t,
                            },
                        ));
                    }
                }
            }
            for (field, name, categorical) in &fields {
                let held = portion.value(*field);
                let compatible = held.is_none_or(|value| matches!(value, crate::model::schedule::OpeningValue::Category(_)) == *categorical);
                let display_name = if compatible {
                    name.clone()
                } else {
                    tr!("inventory-field-incompatible", field = name.clone())
                };
                if *categorical {
                    // The values the models were measured holding, plus
                    // whatever this portion already says: a label that no
                    // longer occurs is kept rather than quietly cleared.
                    let mut options: Vec<Option<String>> = vec![None];
                    options.extend(categories.get(field).into_iter().flatten().cloned().map(Some));
                    if let Some(crate::model::schedule::OpeningValue::Category(label)) = &held
                        && !options.contains(&Some(label.clone()))
                    {
                        options.push(Some(label.clone()));
                    }
                    let mut chosen = match &held {
                        Some(crate::model::schedule::OpeningValue::Category(label)) => Some(label.clone()),
                        _ => None,
                    };
                    let before = chosen.clone();
                    let label = |value: &Option<String>| value.clone().unwrap_or_else(|| tr!("inventory-portion-missing"));
                    let (cell, response) = grid_named_row(ui, &display_name, 1);
                    if !compatible {
                        response.on_hover_text(tr!("inventory-field-incompatible-note"));
                    }
                    if cell.is_positive() {
                        let id = egui::Id::new(("lot_category", portion.id.0, field.0));
                        let selected = label(&chosen);
                        ui.scope_builder(egui::UiBuilder::new().id_salt(id).max_rect(cell), |ui| {
                            ui.set_clip_rect(ui.clip_rect().intersect(cell));
                            ui.spacing_mut().interact_size.y = cell.height();
                            egui::ComboBox::from_id_salt(id.with("combo"))
                                .selected_text(selected)
                                .width(cell.width())
                                .truncate()
                                .show_ui(ui, |ui| {
                                    for option in &options {
                                        ui.selectable_value(&mut chosen, option.clone(), label(option));
                                    }
                                });
                        });
                    }
                    if chosen != before {
                        edits.push(UiCommand::schedule(
                            session,
                            ScheduleEdit::SetOpeningPortionValue {
                                destination: destination.id,
                                lot: lot.id,
                                portion: portion.id,
                                field: *field,
                                value: chosen.map(crate::model::schedule::OpeningValue::Category),
                            },
                        ));
                    }
                    continue;
                }
                let (cell, response) = grid_named_row(ui, &display_name, 1);
                if !compatible {
                    response.on_hover_text(tr!("inventory-field-incompatible-note"));
                }
                if !cell.is_positive() {
                    continue;
                }
                let position = draft.values.iter().position(|(id, held, _)| *id == portion.id && held == field);
                let position = match position {
                    Some(position) => position,
                    None => {
                        draft.values.push((portion.id, *field, String::new()));
                        draft.values.len() - 1
                    }
                };
                let text = &mut draft.values[position].2;
                let error = parse_lot_value(text).err();
                let response = ui.put(cell, egui::TextEdit::singleline(text).desired_width(cell.width()));
                response.clone().on_hover_text(error.clone().unwrap_or_else(|| tr!("inventory-help")));
                if response.lost_focus()
                    && let Ok(value) = parse_lot_value(text)
                {
                    let current = match &held {
                        Some(crate::model::schedule::OpeningValue::Number(number)) => Some(*number),
                        _ => None,
                    };
                    if value != current {
                        edits.push(UiCommand::schedule(
                            session,
                            ScheduleEdit::SetOpeningPortionValue {
                                destination: destination.id,
                                lot: lot.id,
                                portion: portion.id,
                                field: *field,
                                value: value.map(crate::model::schedule::OpeningValue::Number),
                            },
                        ));
                    }
                }
            }
        }
        let body = ui.available_rect_before_wrap();
        if body.is_positive() {
            let response = ui.interact(body, ui.id().with(("new_portion_space", lot.id.0)), egui::Sense::click());
            context_menu_popup(&response, tr!("inventory-portions"), |ui| {
                if ContextMenuAction::new(tr!("inventory-new-portion")).show(ui).clicked() {
                    portion_action = Some(PortionAction::Add);
                    ui.close();
                }
            });
        }
    });
    match portion_action {
        Some(PortionAction::Add) => edits.push(UiCommand::schedule(
            session,
            ScheduleEdit::AddOpeningPortion {
                destination: destination.id,
                lot: lot.id,
                tonnes_t: DEFAULT_LOT_TONNES,
            },
        )),
        Some(PortionAction::Delete(portion)) => edits.push(UiCommand::schedule(
            session,
            ScheduleEdit::DeleteOpeningPortion {
                destination: destination.id,
                lot: lot.id,
                portion,
            },
        )),
        None => {}
    }
    commands.append(&mut edits);
}

enum PortionAction {
    Add,
    Delete(crate::model::schedule::OpeningPortionId),
}

/// A portion's tonnes: finite and above zero. Blank is not an answer here -
/// a portion of nothing is not a portion.
fn parse_lot_tonnes(text: &str) -> Result<f64, String> {
    let trimmed = text.trim();
    let Ok(value) = trimmed.parse::<f64>() else {
        return Err(tr!("schedule-calendar-invalid-number"));
    };
    if !value.is_finite() || value <= 0.0 {
        return Err(crate::model::schedule::ScheduleError::InvalidLotTonnes.message());
    }
    Ok(value)
}

/// One numerical property value, or `None` for a field this portion was never
/// measured against - which fails every condition and is not zero.
fn parse_lot_value(text: &str) -> Result<Option<f64>, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let Ok(value) = trimmed.parse::<f64>() else {
        return Err(tr!("schedule-calendar-invalid-number"));
    };
    if !value.is_finite() {
        return Err(crate::model::schedule::ScheduleError::InvalidLotValue.message());
    }
    Ok(Some(value))
}
