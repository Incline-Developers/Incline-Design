//! The Schedule workspace's truck pages: Truck Classes and Trucking Rules.
//!
//! A class is a pool of one type of truck, not a vehicle, and it starts with
//! no trucks in it - the fleet is sized in the Calendar, where availability
//! and utilisation are sized too. What this page owns is the physical truck:
//! how much it carries and how fast it travels loaded and empty.
//!
//! A trucking rule says which classes are *permitted* on which movements. It
//! expresses no preference and carries no order: every matching rule
//! contributes its classes to one union, which is why the rule list here has
//! no move-up and move-down and the destination rule list does.
//!
//! Nothing here constrains a calculated schedule yet. The one sentence that
//! says so lives in a tooltip on the travel-cycle row rather than in a banner
//! over every table.

use super::schedule_destinations::{descendant_selected, scope_key, visible};
use crate::{
    i18n::tr,
    model::{
        Document,
        schedule::{DestinationSelection, LoaderAgentId, LoaderSelection, MovementSourceScope, MovementSourceSelection, SchedulePlan, TruckClassId, destinations, trucking},
    },
    ui::{
        EditorState,
        state::{ScheduleEdit, ScheduleRuleNameDraft, ScheduleTruckClassDraft, UiCommand},
        widgets::{
            context_menu::{ChecklistRow, ContextMenuAction, Tick, checklist_popup, context_menu_popup, context_menu_separator},
            data_grid::{DataGrid, GridRow, PropertyTable, grid_checkbox_row, grid_named_row, grid_row, grid_select_row, grid_separator_row},
            explorer::explorer_note,
            menu,
        },
    },
};

/// The truck classes, in fleet order.
pub(crate) fn draw_class_list(ui: &mut egui::Ui, rect: egui::Rect, editor: &mut EditorState, plan: &SchedulePlan, session: u32, commands: &mut Vec<UiCommand>) {
    let trucks = plan.trucks();
    let mut selected = editor.schedule_selected_truck_class;
    DataGrid::new("schedule_truck_class_list", rect, &tr!("truck-classes"))
        .column_header(&tr!("planning-name"))
        .show(ui, |ui| {
            if trucks.classes.is_empty() {
                explorer_note(ui, tr!("truck-no-classes"));
            }
            for class in &trucks.classes {
                // The rostered fleet beside the name: a class with no trucks in
                // it supplies nothing, and that is the first thing to know
                // about it.
                let label = format!("{} · {} × {} t", class.name, class.calendar.default_units, number(class.payload_t));
                let response = grid_row(ui, GridRow::new(&label).selected(selected == Some(class.id))).on_hover_text(&label);
                if response.clicked() {
                    selected = Some(class.id);
                }
                context_menu_popup(&response, &class.name, |ui| {
                    if ContextMenuAction::new(tr!("truck-duplicate-class")).show(ui).clicked() {
                        commands.push(UiCommand::schedule(session, ScheduleEdit::DuplicateTruckClass(class.id)));
                        ui.close();
                    }
                    if ContextMenuAction::new(tr!("truck-delete-class")).show(ui).clicked() {
                        commands.push(UiCommand::schedule(session, ScheduleEdit::DeleteTruckClass(class.id)));
                        ui.close();
                    }
                });
            }
            let body = ui.available_rect_before_wrap();
            if body.is_positive() {
                let response = ui.interact(body, ui.id().with("new_truck_class_space"), egui::Sense::click());
                context_menu_popup(&response, tr!("truck-classes"), |ui| {
                    if ContextMenuAction::new(tr!("truck-new-class")).show(ui).clicked() {
                        let name = crate::model::schedule::suggested_name(&tr!("truck-default-class-name"), trucks.classes.iter().map(|class| class.name.clone()));
                        commands.push(UiCommand::schedule(session, ScheduleEdit::AddTruckClass { name }));
                        ui.close();
                    }
                });
            }
        });
    if editor.schedule_selected_truck_class != selected {
        editor.schedule_truck_class_draft = None;
    }
    editor.schedule_selected_truck_class = selected;
}

/// The selected class's cells, and what they make of one another.
pub(crate) fn draw_class_properties(ui: &mut egui::Ui, rect: egui::Rect, editor: &mut EditorState, plan: &SchedulePlan, session: u32, commands: &mut Vec<UiCommand>) {
    let trucks = plan.trucks();
    let Some(class) = editor.schedule_selected_truck_class.and_then(|id| trucks.class(id)).cloned() else {
        PropertyTable::new("schedule_truck_class_properties", rect, &tr!("truck-classes")).show(ui, |rows| {
            rows.header(&tr!("planning-property"), &tr!("planning-value"));
            rows.readonly(&tr!("planning-name"), &tr!("truck-select-class"), None, None);
        });
        return;
    };
    let source = (
        class.name.clone(),
        class.payload_t.to_bits(),
        class.loaded_speed_kph.to_bits(),
        class.unloaded_speed_kph.to_bits(),
    );
    let haul_source = serde_json::to_string(&class).expect("valid truck class");
    if editor
        .schedule_truck_class_draft
        .as_ref()
        .is_none_or(|draft| draft.id != class.id || draft.source != source || draft.haul_source != haul_source)
    {
        editor.schedule_truck_class_draft = Some(ScheduleTruckClassDraft {
            id: class.id,
            source,
            name: class.name.clone(),
            payload: number(class.payload_t),
            haul_source,
            maximum_speed: number(class.maximum_speed_kph),
            maximum_grade: number(class.maximum_grade * 100.0),
            dump_time: number(class.dump_time_s),
            grade_rows: class
                .grade_speeds
                .iter()
                .map(|r| (number(r.from_grade * 100.0), number(r.loaded_kph), number(r.empty_kph)))
                .collect(),
        });
    }
    let taken: Vec<String> = trucks.classes.iter().filter(|other| other.id != class.id).map(|other| other.name.clone()).collect();
    let draft = editor.schedule_truck_class_draft.as_mut().expect("just ensured");
    let name_error = name_problem(&draft.name, taken.iter().cloned());
    let payload_error = parse_positive(&draft.payload, crate::model::schedule::ScheduleError::InvalidPayload).err();

    let mut edits = Vec::new();
    PropertyTable::new("schedule_truck_class_properties", rect, &class.name).show(ui, |rows| {
        rows.header(&tr!("planning-property"), &tr!("planning-value"));
        let response = rows.field(&tr!("planning-name"), &mut draft.name, name_error.as_deref());
        if response.lost_focus() && name_error.is_none() && draft.name.trim() != class.name {
            edits.push(UiCommand::schedule(
                session,
                ScheduleEdit::RenameTruckClass {
                    class: class.id,
                    name: draft.name.trim().to_owned(),
                },
            ));
        }
        let response = rows.field(&tr!("truck-payload"), &mut draft.payload, payload_error.as_deref());
        if response.lost_focus()
            && let Ok(payload) = parse_positive(&draft.payload, crate::model::schedule::ScheduleError::InvalidPayload)
            && payload != class.payload_t
        {
            edits.push(UiCommand::schedule(
                session,
                ScheduleEdit::SetTruckClassPayload {
                    class: class.id,
                    payload_t: payload,
                },
            ));
        }
        let mut commit = false;
        let positive = |text: &str| text.trim().parse::<f64>().ok().filter(|v| v.is_finite() && *v > 0.0);
        let speed_error = positive(&draft.maximum_speed).is_none().then(|| tr!("truck-error-invalid-speed"));
        let grade_error = positive(&draft.maximum_grade).filter(|v| *v <= 100.0).is_none().then(|| tr!("haul-error-grade"));
        let dump_error = draft
            .dump_time
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite() && *v >= 0.0)
            .is_none()
            .then(|| tr!("haul-error-seconds"));
        for (label, value, error) in [
            (tr!("haul-maximum-speed"), &mut draft.maximum_speed, speed_error),
            (tr!("haul-maximum-grade"), &mut draft.maximum_grade, grade_error),
            (tr!("haul-dump-time"), &mut draft.dump_time, dump_error),
        ] {
            commit |= rows.field(&label, value, error.as_deref()).lost_focus();
        }
        rows.three_headers([&tr!("haul-grade-from"), &tr!("haul-loaded-speed"), &tr!("haul-empty-speed")]);
        let removable = draft.grade_rows.len() > 1;
        let mut remove = None;
        for (index, (grade, loaded, empty)) in draft.grade_rows.iter_mut().enumerate() {
            let (responses, removed) = rows.three_fields([grade, loaded, empty], removable);
            commit |= responses.iter().any(|r| r.lost_focus());
            if removed {
                remove = Some(index);
            }
        }
        if let Some(index) = remove {
            draft.grade_rows.remove(index);
            commit = true;
        }
        let mut grades: Vec<_> = draft.grade_rows.iter().filter_map(|r| r.0.trim().parse::<f64>().ok()).collect();
        grades.sort_by(f64::total_cmp);
        let bands_valid = grades.len() == draft.grade_rows.len()
            && grades.windows(2).all(|g| g[0] < g[1])
            && draft.grade_rows.iter().all(|r| positive(&r.1).is_some() && positive(&r.2).is_some());
        if !bands_valid {
            rows.readonly("", &tr!("haul-error-bands"), None, Some(&tr!("haul-error-bands")));
        }
        if rows.action("", &tr!("haul-add-band")).clicked() {
            let grade = grades.last().copied().unwrap_or(6.0) + 2.0;
            let (loaded, empty) = draft.grade_rows.last().map_or(("10".to_owned(), "20".to_owned()), |r| (r.1.clone(), r.2.clone()));
            draft.grade_rows.push((number(grade), loaded, empty));
            commit = true;
        }
        // A class migrated from the old two-speed model holds one speed at
        // every grade; this is the quick way onto grade-dependent speeds.
        if rows.action("", &tr!("haul-generic-speeds")).on_hover_text(tr!("haul-speeds-help")).clicked() {
            draft.grade_rows = trucking::generic_grade_speeds()
                .iter()
                .map(|r| (number(r.from_grade * 100.0), number(r.loaded_kph), number(r.empty_kph)))
                .collect();
            commit = true;
        }
        rows.readonly("", &tr!("haul-speeds-help"), None, None).on_hover_text(tr!("haul-speeds-help"));
        if commit {
            let parsed = (|| -> Option<_> {
                let maximum_speed_kph = draft.maximum_speed.parse::<f64>().ok()?;
                let maximum_grade = draft.maximum_grade.parse::<f64>().ok()? / 100.0;
                let dump_time_s = draft.dump_time.parse::<f64>().ok()?;
                let mut speeds: Vec<trucking::GradeSpeed> = draft
                    .grade_rows
                    .iter()
                    .map(|(g, l, e)| {
                        Some(trucking::GradeSpeed {
                            from_grade: g.parse::<f64>().ok()? / 100.0,
                            loaded_kph: l.parse().ok()?,
                            empty_kph: e.parse().ok()?,
                        })
                    })
                    .collect::<Option<_>>()?;
                speeds.sort_by(|a, b| a.from_grade.total_cmp(&b.from_grade));
                Some((speeds, maximum_speed_kph, maximum_grade, dump_time_s))
            })();
            if let Some((speeds, maximum_speed_kph, maximum_grade, dump_time_s)) = parsed
                && (speeds != class.grade_speeds || maximum_speed_kph != class.maximum_speed_kph || maximum_grade != class.maximum_grade || dump_time_s != class.dump_time_s)
            {
                edits.push(UiCommand::schedule(
                    session,
                    ScheduleEdit::SetTruckClassHaulage {
                        class: class.id,
                        speeds,
                        maximum_speed_kph,
                        maximum_grade,
                        dump_time_s,
                    },
                ));
            }
        }
    });
    commands.append(&mut edits);
}

/// The trucking rules. Unordered by design; see the module documentation.
pub(crate) fn draw_rule_list(ui: &mut egui::Ui, rect: egui::Rect, editor: &mut EditorState, plan: &SchedulePlan, session: u32, commands: &mut Vec<UiCommand>) {
    let trucks = plan.trucks();
    let mut selected = editor.schedule_selected_truck_rule;
    DataGrid::new("schedule_truck_rule_list", rect, &tr!("truck-rules"))
        .column_header(&tr!("planning-name"))
        .show(ui, |ui| {
            if trucks.rules.is_empty() {
                explorer_note(ui, tr!("truck-no-rules"));
            }
            for rule in &trucks.rules {
                let classes: Vec<String> = rule
                    .classes
                    .iter()
                    .map(|id| trucks.class(*id).map(|class| class.name.clone()).unwrap_or_else(|| tr!("truck-error-unknown-class")))
                    .collect();
                let label = format!("{} → {}", rule.name, classes.join(", "));
                let disabled = (!rule.enabled).then(|| tr!("truck-rule-disabled"));
                let response = grid_row(ui, GridRow::new(&label).error(disabled.as_deref()).selected(selected == Some(rule.id))).on_hover_text(&label);
                if response.clicked() {
                    selected = Some(rule.id);
                }
                context_menu_popup(&response, &rule.name, |ui| {
                    if ContextMenuAction::new(tr!("truck-duplicate-rule")).show(ui).clicked() {
                        commands.push(UiCommand::schedule(session, ScheduleEdit::DuplicateTruckingRule(rule.id)));
                        ui.close();
                    }
                    if ContextMenuAction::new(tr!("truck-delete-rule")).show(ui).clicked() {
                        commands.push(UiCommand::schedule(session, ScheduleEdit::DeleteTruckingRule(rule.id)));
                        ui.close();
                    }
                });
            }
            let body = ui.available_rect_before_wrap();
            if body.is_positive() {
                let response = ui.interact(body, ui.id().with("new_trucking_rule_space"), egui::Sense::click());
                context_menu_popup(&response, tr!("truck-rules"), |ui| {
                    // A rule has to permit at least one class, so there is
                    // nothing to add until there is a class to permit.
                    let first = trucks.classes.first().map(|class| class.id);
                    if ContextMenuAction::new(tr!("truck-new-rule")).enabled(first.is_some()).show(ui).clicked() {
                        if let Some(first) = first {
                            let name = crate::model::schedule::suggested_name(&tr!("truck-default-rule-name"), trucks.rules.iter().map(|rule| rule.name.clone()));
                            commands.push(UiCommand::schedule(session, ScheduleEdit::AddTruckingRule { name, classes: vec![first] }));
                        }
                        ui.close();
                    }
                });
            }
        });
    if editor.schedule_selected_truck_rule != selected {
        editor.schedule_truck_rule_draft = None;
    }
    editor.schedule_selected_truck_rule = selected;
}

/// The selected trucking rule: what it describes, and what it permits.
pub(crate) fn draw_rule_editor(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    editor: &mut EditorState,
    plan: &SchedulePlan,
    document: &Document,
    session: u32,
    commands: &mut Vec<UiCommand>,
) {
    let trucks = plan.trucks();
    let Some(rule) = editor.schedule_selected_truck_rule.and_then(|id| trucks.rule(id)).cloned() else {
        PropertyTable::new("schedule_truck_rule_editor", rect, &tr!("truck-rule")).show(ui, |rows| {
            rows.header(&tr!("planning-property"), &tr!("planning-value"));
            rows.readonly(&tr!("planning-name"), &tr!("truck-select-rule"), None, None);
        });
        return;
    };
    if editor
        .schedule_truck_rule_draft
        .as_ref()
        .is_none_or(|draft| draft.id != rule.id || draft.source != rule.name)
    {
        editor.schedule_truck_rule_draft = Some(ScheduleRuleNameDraft {
            id: rule.id,
            source: rule.name.clone(),
            name: rule.name.clone(),
        });
    }
    let available = destinations::available(document.solids(), plan.routing());
    let stockpiles: Vec<_> = available
        .iter()
        .filter(|entry| entry.kind == crate::model::schedule::DestinationKind::Stockpile)
        .cloned()
        .collect();
    let sources = editor.schedule_routing_sources.clone();
    let mut collapsed = editor.schedule_source_collapsed.clone();
    let taken: Vec<String> = trucks.rules.iter().filter(|other| other.id != rule.id).map(|other| other.name.clone()).collect();
    let classes = trucks.classes.clone();
    let mut edits = Vec::new();
    let draft = editor.schedule_truck_rule_draft.as_mut().expect("just ensured");
    let name_error = name_problem(&draft.name, taken.iter().cloned());
    DataGrid::new("schedule_truck_rule_editor", rect, &rule.name)
        .column_header(&tr!("truck-rule"))
        .show(ui, |ui| {
            grid_separator_row(ui, &tr!("truck-rule"), 0);
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
                            ScheduleEdit::RenameTruckingRule {
                                rule: rule.id,
                                name: draft.name.trim().to_owned(),
                            },
                        ));
                    }
                }
            }
            let mut enabled = rule.enabled;
            if grid_checkbox_row(ui, &tr!("truck-rule-enabled"), &mut enabled, 0) {
                edits.push(UiCommand::schedule(session, ScheduleEdit::SetTruckingRuleEnabled { rule: rule.id, enabled }));
            }

            // Loaders.
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
                let response = grid_select_row(ui, ("truck_rule_loaders", rule.id.0), &tr!("truck-rule-loaders"), &summary, 0);
                let mut next: Option<LoaderSelection> = None;
                checklist_popup(&response, tr!("truck-rule-loaders"), 240.0, |ui| {
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
                            // The last tick stays until All is chosen; see the
                            // destination rule editor for why.
                            next = (!list.is_empty()).then_some(LoaderSelection::Only(list));
                        }
                    }
                });
                if let Some(loaders) = next
                    && loaders != rule.loaders
                    && !matches!(&loaders, LoaderSelection::Only(agents) if agents.is_empty())
                {
                    edits.push(UiCommand::schedule(session, ScheduleEdit::SetTruckingRuleLoaders { rule: rule.id, loaders }));
                }
            }

            // Sources: pit ground and stockpiles in one picker, because a rule
            // may name either and the reclaim it is authored for arrives later.
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
                let response = grid_select_row(ui, ("truck_rule_sources", rule.id.0), &tr!("truck-rule-sources"), &summary, 0);
                let mut next: Option<MovementSourceSelection> = None;
                let toggle = |held: &[MovementSourceScope],
                              scope: MovementSourceScope,
                              picked: bool,
                              everything: &dyn Fn() -> Vec<MovementSourceScope>|
                 -> Option<MovementSourceSelection> {
                    let mut list: Vec<MovementSourceScope> = if held.is_empty() { everything() } else { held.to_vec() };
                    if picked {
                        list.retain(|entry| *entry != scope);
                    } else {
                        list.push(scope);
                    }
                    (!list.is_empty()).then_some(MovementSourceSelection::Only(list))
                };
                let everything = || -> Vec<MovementSourceScope> {
                    sources
                        .iter()
                        .filter(|view| view.depth == 0)
                        .map(|view| MovementSourceScope::Ground(view.scope))
                        .chain(stockpiles.iter().map(|entry| MovementSourceScope::Stockpile(entry.id)))
                        .collect()
                };
                checklist_popup(&response, tr!("truck-rule-sources"), 260.0, |ui| {
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
                    menu::panel_section(ui, tr!("truck-rule-ground"));
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
                            next = toggle(if all { &[] } else { &held }, scope, picked, &everything);
                        }
                    }
                    context_menu_separator(ui);
                    menu::panel_section(ui, tr!("truck-rule-stockpiles"));
                    if stockpiles.is_empty() {
                        menu::menu_note(ui, tr!("destination-no-stockpiles"));
                    }
                    for entry in &stockpiles {
                        let scope = MovementSourceScope::Stockpile(entry.id);
                        let picked = all || held.contains(&scope);
                        if ChecklistRow::new(&entry.name, Tick::of(picked, false)).depth(1).show(ui).toggled {
                            next = toggle(if all { &[] } else { &held }, scope, picked, &everything);
                        }
                    }
                });
                if let Some(sources) = next
                    && sources != rule.sources
                {
                    edits.push(UiCommand::schedule(session, ScheduleEdit::SetTruckingRuleSources { rule: rule.id, sources }));
                }
            }

            // Destinations.
            {
                let all = matches!(rule.destinations, DestinationSelection::All);
                let held: Vec<_> = match &rule.destinations {
                    DestinationSelection::All => Vec::new(),
                    DestinationSelection::Only(ids) => ids.clone(),
                };
                let summary = if all {
                    tr!("truck-rule-all-destinations")
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
                let response = grid_select_row(ui, ("truck_rule_destinations", rule.id.0), &tr!("truck-rule-destinations"), &summary, 0);
                let mut next: Option<DestinationSelection> = None;
                checklist_popup(&response, tr!("truck-rule-destinations"), 260.0, |ui| {
                    if ChecklistRow::new(&tr!("truck-rule-all-destinations"), Tick::of(all, false)).show(ui).toggled {
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
                        let label = format!("{} · {}", entry.name, entry.kind.label());
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
                    edits.push(UiCommand::schedule(session, ScheduleEdit::SetTruckingRuleDestinations { rule: rule.id, destinations }));
                }
            }

            // Classes. No "All" here: a rule permits the classes it names, and
            // "whatever is on site" is a fleet decision rather than a rule.
            grid_separator_row(ui, &tr!("truck-rule-classes"), 0);
            {
                let summary = rule
                    .classes
                    .iter()
                    .map(|id| {
                        classes
                            .iter()
                            .find(|class| class.id == *id)
                            .map(|class| class.name.clone())
                            .unwrap_or_else(|| tr!("truck-error-unknown-class"))
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let response = grid_select_row(ui, ("truck_rule_classes", rule.id.0), &tr!("truck-rule-classes"), &summary, 0);
                let mut chosen: Vec<TruckClassId> = rule.classes.clone();
                checklist_popup(&response, tr!("truck-rule-classes"), 240.0, |ui| {
                    if classes.is_empty() {
                        menu::menu_note(ui, tr!("truck-rule-no-classes"));
                        return;
                    }
                    let all = classes.iter().all(|class| chosen.contains(&class.id));
                    let any = classes.iter().any(|class| chosen.contains(&class.id));
                    if ChecklistRow::new(&tr!("destination-select-all"), Tick::of(all, any)).show(ui).toggled && !all {
                        chosen = classes.iter().map(|class| class.id).collect();
                    }
                    context_menu_separator(ui);
                    for class in &classes {
                        let picked = chosen.contains(&class.id);
                        if ChecklistRow::new(&class.name, Tick::of(picked, false)).depth(1).show(ui).toggled {
                            if picked {
                                chosen.retain(|held| *held != class.id);
                            } else {
                                chosen.push(class.id);
                            }
                        }
                    }
                });
                // The last class stays: a rule permitting nothing would match
                // movements and leave them with no permitted truck at all.
                if !chosen.is_empty() && chosen != rule.classes {
                    edits.push(UiCommand::schedule(session, ScheduleEdit::SetTruckingRuleClasses { rule: rule.id, classes: chosen }));
                }
            }
        });
    editor.schedule_source_collapsed = collapsed;
    commands.append(&mut edits);
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

/// Parse a strictly positive figure, naming the domain error it breaks.
pub(crate) fn parse_positive(text: &str, error: crate::model::schedule::ScheduleError) -> Result<f64, String> {
    let trimmed = text.trim();
    let Ok(value) = trimmed.parse::<f64>() else {
        return Err(tr!("schedule-rate-not-a-number"));
    };
    if !value.is_finite() || value <= 0.0 {
        return Err(error.message());
    }
    Ok(value)
}

/// A figure as digits, without the trailing zeroes a raw `f64` prints.
pub(crate) fn number(value: f64) -> String {
    let mut text = format!("{value:.3}");
    while text.contains('.') && text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    text
}
