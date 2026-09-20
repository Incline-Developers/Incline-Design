//! The Schedule workspace's Setup subpage: the loader fleet.
//!
//! Two editable lists and a name. *Loader Classes* are machine types with a
//! dig rate; *Loader Agents* are the machines on site, each of one class and
//! taking its rate from that class - so a rate typed once applies to every
//! machine of that type, and the Gantt gains a row the moment an agent is
//! added.
//!
//! Nothing here computes: the lists are configuration, and editing them can
//! neither run the Solids pipeline nor move its completion markers.

use crate::{
    i18n::{tr, tr_format},
    model::{Document, ReserveField, schedule::SchedulePlan},
    ui::{
        EditorState,
        fonts::bold,
        state::{ScheduleAgentDraft, ScheduleBarHeightDraft, ScheduleClassDraft, ScheduleEdit, ScheduleNameDraft, ScheduleStep, UiCommand},
        widgets::{
            context_menu::{ContextMenuAction, context_menu_popup},
            data_grid::{DataGrid, GridRow, PropertyTable, grid_row, property_table_height},
            explorer::{ExplorerEntry, explorer_note},
        },
    },
};

/// Preserve the stored rate, including small or fractional rates.
fn rate_text(rate: f64) -> String {
    rate.to_string()
}

fn rate_with_unit(rate: f64) -> String {
    tr_format!(literal = "%rate% %unit%", rate = rate_text(rate), unit = tr!("schedule-tph"))
}

/// Parse a typed dig rate, or say what is wrong with it.
///
/// Separate from the domain's own check so the field can explain an entry
/// that is not a number at all, which never reaches the domain.
fn parse_rate(text: &str) -> Result<f64, String> {
    let Ok(value) = text.trim().parse::<f64>() else {
        return Err(tr!("schedule-rate-not-a-number"));
    };
    if !value.is_finite() || value <= 0.0 {
        return Err(crate::model::schedule::ScheduleError::InvalidRate.message());
    }
    Ok(value)
}

/// Why this name cannot be committed, for the inline badge beside it. The
/// command layer checks the same rules; this is so the user is told before
/// pressing anything rather than after.
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

/// The Schedule Setup step tree.
///
/// A flat list of four steps with the same badges, the same connecting line
/// and the same context menu as the Solids page's, because it is the same kind
/// of thing: an explicitly executed pipeline, not a category tree. The Site
/// Data grouping the fleet lists used to sit under is gone with it - a step
/// that can be marked Complete, Stale or Failed belongs in the chain that
/// marks it.
///
/// Dig sequences are deliberately absent. An ordered run of ground belongs to
/// the Gantt bar that works it, and is authored there; a second list of them
/// here would be a second place the same thing could be edited.
pub(crate) fn draw_steps(ui: &mut egui::Ui, editor: &mut EditorState, commands: &mut Vec<UiCommand>) {
    let mut step = editor.schedule_setup_step;
    let mut markers = Vec::with_capacity(ScheduleStep::ALL.len());
    for entry in ScheduleStep::ALL {
        let status = &editor.schedule_stages[entry.index()];
        ui.horizontal(|ui| {
            ui.add_space(ui.spacing().indent);
            let entry_response = ExplorerEntry::new(egui::Id::new(entry.tree_id()), bold(&entry.label()))
                .leading_icon(super::planning_setup::step_icon(status.state), super::planning_setup::stage_tint(ui, status.state))
                .header_aligned_icon()
                .selected(step == entry)
                .show(ui);
            if let Some(rect) = entry_response.icon_rect {
                markers.push((rect, status.state));
            }
            let response = entry_response.response;
            if response.clicked() {
                step = entry;
            }
            draw_step_menu(&response, entry, editor.schedule_run_active, commands);
        });
    }
    super::planning_setup::paint_step_links(ui, &markers);
    editor.schedule_setup_step = step;
}

fn draw_step_menu(response: &egui::Response, step: ScheduleStep, running: bool, commands: &mut Vec<UiCommand>) {
    context_menu_popup(response, step.label(), |ui| {
        if ContextMenuAction::new(tr!("stage-run-step")).enabled(!running).show(ui).clicked() {
            commands.push(UiCommand::RunScheduleStage(step));
            ui.close();
        }
        if ContextMenuAction::new(tr!("stage-run-all")).enabled(!running).show(ui).clicked() {
            commands.push(UiCommand::RunAllScheduleStages);
            ui.close();
        }
        if ContextMenuAction::new(tr!("stage-cancel")).enabled(running).show(ui).clicked() {
            commands.push(UiCommand::CancelScheduleRun);
            ui.close();
        }
    });
}

/// The Scheduling Readiness step: what the last completed run checked, and
/// what the Gantt would be told if it asked to calculate right now.
///
/// Reports; it never runs anything. A held result stays on screen after an
/// edit retires it, labelled as belonging to the run that produced it - a
/// result that vanished on the first edit would leave the page with nothing to
/// say about what was checked.
pub(crate) fn draw_readiness(ui: &mut egui::Ui, rect: egui::Rect, editor: &EditorState, plan: &SchedulePlan, document: &Document) {
    use crate::app::planning_pipeline::StageState;

    let status = &editor.schedule_stages[ScheduleStep::Readiness.index()];
    let summary = status.last_success.as_ref();
    let field = plan
        .tonnage_field()
        .map(|id| {
            document
                .reserve_fields()
                .iter()
                .find(|field| field.id == id)
                .map_or_else(|| tr!("sequence-tonnage-field-missing"), |field| field_option_label(document, field))
        })
        .unwrap_or_else(|| tr!("schedule-tonnage-field-none"));
    let result = match summary {
        None => tr!("schedule-readiness-never-run"),
        Some(_) if status.state == StageState::Complete => tr!("schedule-readiness-result-current"),
        Some(_) => tr!("schedule-readiness-result-stale"),
    };
    let blocks = summary.map_or_else(|| tr!(literal = "—"), |summary| summary.entities.to_string());

    let table_rect = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), property_table_height(ui, 5).min(rect.height())));
    PropertyTable::new("schedule_readiness", table_rect, &ScheduleStep::Readiness.label()).show(ui, |rows| {
        rows.header(&tr!("planning-property"), &tr!("planning-value"));
        rows.readonly(&tr!("schedule-readiness-state"), &status.state.label(), None, None);
        rows.readonly(&tr!("schedule-readiness-last-run"), &result, None, None);
        rows.readonly(&tr!("schedule-readiness-tonnage-field"), &field, None, None);
        rows.readonly(&tr!("schedule-readiness-blocks"), &blocks, None, None);
    });

    // Everything the step had to say, in full, and then the one sentence the
    // Gantt would show. Both are statements about a completed run, so neither
    // is offered as something to press.
    let body = egui::Rect::from_min_max(egui::pos2(rect.left() + 8.0, table_rect.bottom() + 8.0), egui::pos2(rect.right() - 8.0, rect.bottom()));
    if !body.is_positive() {
        return;
    }
    ui.scope_builder(egui::UiBuilder::new().max_rect(body), |ui| {
        ui.set_clip_rect(ui.clip_rect().intersect(body));
        egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
            if let Some(message) = &status.message {
                ui.add(egui::Label::new(egui::RichText::new(message).color(ui.visuals().error_fg_color)).wrap());
            }
            for entry in &status.diagnostics {
                let text = match &entry.entity {
                    Some(entity) => tr_format!(literal = "%entity%: %message%", entity = entity.clone(), message = entry.message.clone()),
                    None => entry.message.clone(),
                };
                let color = if entry.blocking { ui.visuals().error_fg_color } else { ui.visuals().weak_text_color() };
                ui.add(egui::Label::new(egui::RichText::new(text).color(color)).wrap());
            }
            if !editor.schedule_calculation_status.is_empty() {
                ui.add_space(8.0);
                ui.add(egui::Label::new(bold(&editor.schedule_calculation_status)).wrap());
            }
        });
    });
}

/// One field as the tonnage combo lists it: its name and how it aggregates,
/// because only a summed field can be read as tonnes and the choice should
/// not look like it can.
fn field_option_label(document: &Document, field: &ReserveField) -> String {
    tr_format!(
        literal = "%name% · %aggregation%",
        name = field.name.clone(),
        aggregation = super::planning_setup::aggregation_summary(document, &field.aggregation)
    )
}

/// The schedule's own settings: what it is called, and which reserve field is
/// read as tonnes.
///
/// The field is chosen, never guessed: the combo offers "Not chosen" and every
/// field in the project's Field List with its aggregation beside it, and the
/// assumption the choice rests on is stated under the table rather than
/// implied. A choice that cannot produce a tonnage is allowed and then
/// explained by the readiness report, because a field can be re-aggregated
/// after it was chosen.
pub(crate) fn draw_configuration(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    editor: &mut EditorState,
    plan: &SchedulePlan,
    document: &Document,
    session: u32,
    commands: &mut Vec<UiCommand>,
) {
    if editor.schedule_name_draft.as_ref().is_none_or(|draft| draft.source != plan.name) {
        editor.schedule_name_draft = Some(ScheduleNameDraft {
            source: plan.name.clone(),
            text: plan.name.clone(),
        });
    }
    let draft = editor.schedule_name_draft.as_mut().expect("just ensured");
    if editor.schedule_bar_height_draft.as_ref().is_none_or(|draft| draft.source != plan.bar_height()) {
        editor.schedule_bar_height_draft = Some(ScheduleBarHeightDraft {
            source: plan.bar_height(),
            text: plan.bar_height().to_string(),
        });
    }
    let height_draft = editor.schedule_bar_height_draft.as_mut().expect("just ensured");
    let parsed_height = height_draft.text.trim().parse::<f32>().ok();
    let height_error = (!parsed_height
        .is_some_and(|height| height.is_finite() && (crate::model::schedule::MIN_BAR_HEIGHT..=crate::model::schedule::MAX_BAR_HEIGHT).contains(&height)))
    .then(|| crate::model::schedule::ScheduleError::InvalidBarHeight.message());
    let mut edits = Vec::new();
    let no_fields = document.reserve_fields().is_empty();
    // Header + schedule name + bar height + scheduling quantity + destination
    // routing, plus the explanatory empty-field row when the project has no
    // reserve schema. The header is a table row too; omitting it from this
    // count clips the quantity combo.
    let rows = 5 + usize::from(no_fields);
    let table_rect = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), property_table_height(ui, rows).min(rect.height())));
    PropertyTable::new("schedule_configuration", table_rect, &tr!("planning-configuration")).show(ui, |rows| {
        rows.header(&tr!("planning-property"), &tr!("planning-value"));
        let response = rows.field(&tr!("planning-schedule-name"), &mut draft.text, None);
        if response.lost_focus() && draft.text.trim() != plan.name {
            edits.push(UiCommand::schedule(session, ScheduleEdit::SetName(draft.text.trim().to_owned())));
        }
        let response = rows.field(&tr!("schedule-bar-height"), &mut height_draft.text, height_error.as_deref());
        if response.lost_focus()
            && let Some(height) = parsed_height
            && height != plan.bar_height()
            && height_error.is_none()
        {
            edits.push(UiCommand::schedule(session, ScheduleEdit::SetBarHeight(height)));
        }
        let mut tonnage = plan.tonnage_field();
        let selected_text = tonnage
            .and_then(|id| document.reserve_fields().iter().find(|field| field.id == id))
            .map(|field| field_option_label(document, field))
            .or_else(|| tonnage.map(|_| tr!("sequence-tonnage-field-missing")))
            .unwrap_or_else(|| tr!("schedule-tonnage-field-none"));
        let response = rows.combo(
            "schedule_tonnage_field",
            &tr!("schedule-tonnage-field"),
            &mut tonnage,
            &selected_text,
            std::iter::once((None, tr!("schedule-tonnage-field-none"))).chain(document.reserve_fields().iter().map(|field| (Some(field.id), field_option_label(document, field)))),
        );
        if response.changed() && tonnage != plan.tonnage_field() {
            edits.push(UiCommand::schedule(session, ScheduleEdit::SetTonnageField(tonnage)));
        }
        // Routing is opt-in and stays opt-in: configuring destinations and
        // writing rules changes nothing until this is switched on, so a project
        // that has never seen this page keeps the dig-only behaviour it was
        // authored against.
        let mut routing = plan.routing().enabled;
        if rows.checkbox(&tr!("destination-routing-enabled"), &mut routing).changed() {
            edits.push(UiCommand::schedule(session, ScheduleEdit::SetRoutingEnabled(routing)));
        }
        if no_fields {
            rows.readonly("", &tr!("schedule-tonnage-field-no-fields"), None, None);
        }
    });
    commands.extend(edits);
}

/// The Loader Classes list. Selecting one drives the property table beside it.
pub(crate) fn draw_class_list(ui: &mut egui::Ui, rect: egui::Rect, editor: &mut EditorState, plan: &SchedulePlan, session: u32, commands: &mut Vec<UiCommand>) {
    let mut selected = editor.schedule_selected_class;
    let mut open_dialog = false;
    DataGrid::new("schedule_class_list", rect, &tr!("schedule-loader-classes"))
        .column_header(&tr!("planning-name"))
        .show(ui, |ui| {
            for class in plan.classes() {
                let label = tr_format!(literal = "%name% · %rate%", name = class.name.clone(), rate = rate_with_unit(class.default_dig_rate_tph));
                let response = grid_row(ui, GridRow::new(&label).selected(selected == Some(class.id))).on_hover_text(&label);
                if response.clicked() {
                    selected = Some(class.id);
                }
                context_menu_popup(&response, &class.name, |ui| {
                    if ContextMenuAction::new(tr!("schedule-delete-class")).show(ui).clicked() {
                        commands.push(UiCommand::schedule(session, ScheduleEdit::DeleteClass(class.id)));
                        ui.close();
                    }
                });
            }
            let body = ui.available_rect_before_wrap();
            if body.is_positive() {
                let response = ui.interact(body, ui.id().with("new_loader_class_space"), egui::Sense::click());
                context_menu_popup(&response, tr!("schedule-loader-classes"), |ui| {
                    if ContextMenuAction::new(tr!("schedule-new-class")).show(ui).clicked() {
                        open_dialog = true;
                        ui.close();
                    }
                });
            }
        });
    if editor.schedule_selected_class != selected {
        // A different class is being edited; its cells replace whatever was
        // half-typed into the previous one.
        editor.schedule_class_draft = None;
    }
    editor.schedule_selected_class = selected;
    if open_dialog && !editor.new_loader_class_open {
        // Seeded as the dialog opens, not while it is on screen: a name
        // refilled every frame could never be cleared and retyped.
        editor.new_loader_class_name = suggested_class_name(plan);
        editor.new_loader_class_rate.clear();
        editor.new_loader_class_open = true;
    }
}

/// The selected class's editable cells: its name, and the rate every machine
/// of this type digs at.
pub(crate) fn draw_class_properties(ui: &mut egui::Ui, rect: egui::Rect, editor: &mut EditorState, plan: &SchedulePlan, session: u32, commands: &mut Vec<UiCommand>) {
    let Some(class) = editor.schedule_selected_class.and_then(|id| plan.class(id)) else {
        PropertyTable::new("schedule_class_properties", rect, &tr!("schedule-loader-classes")).show(ui, |rows| {
            rows.header(&tr!("planning-property"), &tr!("planning-value"));
            rows.readonly(&tr!("planning-name"), &tr!("schedule-select-class"), None, None);
        });
        return;
    };
    let source = (class.name.clone(), class.default_dig_rate_tph);
    if editor.schedule_class_draft.as_ref().is_none_or(|draft| draft.id != class.id || draft.source != source) {
        editor.schedule_class_draft = Some(ScheduleClassDraft {
            id: class.id,
            name: source.0.clone(),
            rate: source.1.to_string(),
            source,
        });
    }
    let draft = editor.schedule_class_draft.as_mut().expect("just ensured");
    let name_error = name_problem(&draft.name, plan.classes().iter().filter(|other| other.id != class.id).map(|other| other.name.clone()));
    let rate_error = parse_rate(&draft.rate).err();
    let users = plan.agents_of(class.id).count();
    let mut edits = Vec::new();
    PropertyTable::new("schedule_class_properties", rect, &class.name).show(ui, |rows| {
        rows.header(&tr!("planning-property"), &tr!("planning-value"));
        let name = rows.field(&tr!("planning-name"), &mut draft.name, name_error.as_deref());
        if name.lost_focus()
            && name_problem(&draft.name, plan.classes().iter().filter(|other| other.id != class.id).map(|other| other.name.clone())).is_none()
            && draft.name.trim() != class.name
        {
            edits.push(UiCommand::schedule(
                session,
                ScheduleEdit::RenameClass {
                    class: class.id,
                    name: draft.name.trim().to_owned(),
                },
            ));
        }
        let rate = rows.field(&tr!("schedule-dig-rate"), &mut draft.rate, rate_error.as_deref());
        if rate.lost_focus()
            && let Ok(value) = parse_rate(&draft.rate)
            && value != class.default_dig_rate_tph
        {
            edits.push(UiCommand::schedule(session, ScheduleEdit::SetClassRate { class: class.id, rate_tph: value }));
        }
        // Named here rather than only on deletion: a class in use cannot be
        // deleted, and knowing that before trying is the point.
        rows.readonly(&tr!("schedule-loader-agents"), &users.to_string(), None, None);
    });
    commands.append(&mut edits);
}

/// The Loader Agents list: the machines themselves, each showing its class.
pub(crate) fn draw_agent_list(ui: &mut egui::Ui, rect: egui::Rect, editor: &mut EditorState, plan: &SchedulePlan, session: u32, commands: &mut Vec<UiCommand>) {
    let mut selected = editor.schedule_selected_agent;
    let mut open_dialog = false;
    let has_classes = !plan.classes().is_empty();
    DataGrid::new("schedule_agent_list", rect, &tr!("schedule-loader-agents"))
        .column_header(&tr!("planning-name"))
        .show(ui, |ui| {
            if !has_classes {
                explorer_note(ui, tr!("schedule-no-classes"));
            } else if plan.agents().is_empty() {
                explorer_note(ui, tr!("schedule-no-agents"));
            }
            for agent in plan.agents() {
                let class = plan
                    .class(agent.class_id)
                    .map(|class| class.name.clone())
                    .unwrap_or_else(|| tr!("schedule-error-unknown-class"));
                let label = tr_format!(literal = "%name% · %class%", name = agent.name.clone(), class = class);
                let response = grid_row(ui, GridRow::new(&label).selected(selected == Some(agent.id))).on_hover_text(&label);
                if response.clicked() {
                    selected = Some(agent.id);
                }
                context_menu_popup(&response, &agent.name, |ui| {
                    if ContextMenuAction::new(tr!("schedule-delete-agent")).show(ui).clicked() {
                        commands.push(UiCommand::schedule(session, ScheduleEdit::DeleteAgent(agent.id)));
                        ui.close();
                    }
                });
            }
            let body = ui.available_rect_before_wrap();
            if body.is_positive() {
                let response = ui.interact(body, ui.id().with("new_loader_agent_space"), egui::Sense::click());
                context_menu_popup(&response, tr!("schedule-loader-agents"), |ui| {
                    if ContextMenuAction::new(tr!("schedule-new-agent")).enabled(has_classes).show(ui).clicked() {
                        open_dialog = true;
                        ui.close();
                    }
                });
            }
        });
    if editor.schedule_selected_agent != selected {
        editor.schedule_agent_draft = None;
    }
    editor.schedule_selected_agent = selected;
    if open_dialog && !editor.new_loader_agent_open {
        editor.new_loader_agent_name = suggested_agent_name(plan);
        editor.new_loader_agent_class = plan.classes().first().map(|class| class.id);
        editor.new_loader_agent_open = true;
    }
}

/// The selected agent's cells: its name, its class, and the rate that class
/// gives it. The rate is shown but not editable - stage 1 has no per-machine
/// override, and offering one here would imply the class rate was a default
/// this row had departed from.
pub(crate) fn draw_agent_properties(ui: &mut egui::Ui, rect: egui::Rect, editor: &mut EditorState, plan: &SchedulePlan, session: u32, commands: &mut Vec<UiCommand>) {
    let Some(agent) = editor.schedule_selected_agent.and_then(|id| plan.agent(id)) else {
        PropertyTable::new("schedule_agent_properties", rect, &tr!("schedule-loader-agents")).show(ui, |rows| {
            rows.header(&tr!("planning-property"), &tr!("planning-value"));
            rows.readonly(&tr!("planning-name"), &tr!("schedule-select-agent"), None, None);
        });
        return;
    };
    if editor.schedule_agent_draft.as_ref().is_none_or(|draft| draft.id != agent.id || draft.source != agent.name) {
        editor.schedule_agent_draft = Some(ScheduleAgentDraft {
            id: agent.id,
            source: agent.name.clone(),
            name: agent.name.clone(),
        });
    }
    let draft = editor.schedule_agent_draft.as_mut().expect("just ensured");
    let name_error = name_problem(&draft.name, plan.agents().iter().filter(|other| other.id != agent.id).map(|other| other.name.clone()));
    let selected_text = plan
        .class(agent.class_id)
        .map(|class| class.name.clone())
        .unwrap_or_else(|| tr!("schedule-error-unknown-class"));
    let rate = plan.effective_rate_tph(agent.id).map(rate_text).unwrap_or_else(|| tr!("schedule-error-unknown-class"));
    let mut class_choice = agent.class_id;
    let mut edits = Vec::new();
    PropertyTable::new("schedule_agent_properties", rect, &agent.name).show(ui, |rows| {
        rows.header(&tr!("planning-property"), &tr!("planning-value"));
        let name = rows.field(&tr!("planning-name"), &mut draft.name, name_error.as_deref());
        if name.lost_focus()
            && name_problem(&draft.name, plan.agents().iter().filter(|other| other.id != agent.id).map(|other| other.name.clone())).is_none()
            && draft.name.trim() != agent.name
        {
            edits.push(UiCommand::schedule(
                session,
                ScheduleEdit::RenameAgent {
                    agent: agent.id,
                    name: draft.name.trim().to_owned(),
                },
            ));
        }
        let response = rows.combo(
            "schedule_agent_class",
            &tr!("schedule-class"),
            &mut class_choice,
            &selected_text,
            plan.classes().iter().map(|class| (class.id, class.name.clone())),
        );
        if response.changed() && class_choice != agent.class_id {
            edits.push(UiCommand::schedule(
                session,
                ScheduleEdit::SetAgentClass {
                    agent: agent.id,
                    class: class_choice,
                },
            ));
        }
        rows.readonly(&tr!("schedule-effective-rate"), &rate, Some(&tr!("schedule-tph")), None);
    });
    commands.append(&mut edits);
}

/// A name for a new class or agent that nothing in the fleet already has.
pub(crate) fn suggested_class_name(plan: &SchedulePlan) -> String {
    crate::model::schedule::suggested_name(&tr!("schedule-loader-class-default"), plan.classes().iter().map(|class| class.name.clone()))
}

pub(crate) fn suggested_agent_name(plan: &SchedulePlan) -> String {
    crate::model::schedule::suggested_name(&tr!("schedule-loader-agent-default"), plan.agents().iter().map(|agent| agent.name.clone()))
}
