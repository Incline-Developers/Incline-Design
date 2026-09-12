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
        state::{ScheduleAgentDraft, ScheduleClassDraft, ScheduleEdit, ScheduleNameDraft, ScheduleSection, ScheduleSequenceDraft, UiCommand},
        unthemed_icon,
        widgets::{
            context_menu::{ContextMenuAction, context_menu_popup},
            data_grid::{DataGrid, GridRow, PropertyTable, grid_row, property_table_height},
            explorer::{ExplorerEntry, ExplorerHeader, explorer_note},
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

/// The Schedule Setup step tree: the schedule's own settings, the two fleet
/// lists under Site Data, then the dig sequences those machines will work.
pub(crate) fn draw_steps(ui: &mut egui::Ui, editor: &mut EditorState) {
    let mut section = editor.schedule_section;
    let mut entry = |ui: &mut egui::Ui, id: &'static str, label: String, value: ScheduleSection| {
        ui.horizontal(|ui| {
            ui.add_space(ui.spacing().indent);
            if ExplorerEntry::new(egui::Id::new(id), bold(&label))
                .leading_icon(unthemed_icon!("step_pending.svg"), egui::Color32::WHITE)
                .header_aligned_icon()
                .selected(section == value)
                .show(ui)
                .response
                .clicked()
            {
                section = value;
            }
        });
    };
    entry(ui, "schedule_configuration", tr!("planning-configuration"), ScheduleSection::Configuration);
    ExplorerHeader::new(egui::Id::new("schedule_site_data"), tr!("planning-site-data"))
        .icon(unthemed_icon!("step_pending.svg"))
        .show(ui, |ui| {
            entry(ui, "schedule_loader_classes", tr!("schedule-loader-classes"), ScheduleSection::LoaderClasses);
            entry(ui, "schedule_loader_agents", tr!("schedule-loader-agents"), ScheduleSection::LoaderAgents);
        });
    entry(ui, "schedule_sequences", tr!("schedule-sequences"), ScheduleSection::Sequences);
    editor.schedule_section = section;
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
    let mut edits = Vec::new();
    let no_fields = document.reserve_fields().is_empty();
    let rows = 2 + usize::from(no_fields);
    let table_rect = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), property_table_height(ui, rows).min(rect.height())));
    PropertyTable::new("schedule_configuration", table_rect, &tr!("planning-configuration")).show(ui, |rows| {
        rows.header(&tr!("planning-property"), &tr!("planning-value"));
        let response = rows.field(&tr!("planning-schedule-name"), &mut draft.text, None);
        if response.lost_focus() && draft.text.trim() != plan.name {
            edits.push(UiCommand::schedule(session, ScheduleEdit::SetName(draft.text.trim().to_owned())));
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
        if no_fields {
            rows.readonly("", &tr!("schedule-tonnage-field-no-fields"), None, None);
        }
    });
    // The unit assumption the tonnage choice rests on, stated on screen where
    // the choice is made rather than in a readiness problem later.
    let note_rect = egui::Rect::from_min_max(egui::pos2(rect.left() + 8.0, table_rect.bottom() + 8.0), egui::pos2(rect.right() - 8.0, rect.bottom()));
    if note_rect.is_positive() {
        ui.scope_builder(egui::UiBuilder::new().max_rect(note_rect), |ui| {
            ui.set_clip_rect(ui.clip_rect().intersect(note_rect));
            ui.add(egui::Label::new(egui::RichText::new(tr!("schedule-tonnage-field-note")).weak().small()).wrap());
        });
    }
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

pub(crate) fn suggested_sequence_name(plan: &SchedulePlan) -> String {
    crate::model::schedule::suggested_name(&tr!("schedule-sequence-default-name"), plan.sequences().iter().map(|sequence| sequence.name.clone()))
}

/// The Dig Sequences list. A row shows the sequence's name and, once measured,
/// its total tonnes; what stands between it and that figure is the selected
/// row's own panel beside this list.
pub(crate) fn draw_sequence_list(ui: &mut egui::Ui, rect: egui::Rect, editor: &mut EditorState, plan: &SchedulePlan, session: u32, commands: &mut Vec<UiCommand>) {
    let mut selected = editor.schedule_selected_sequence;
    let mut open_dialog = false;
    DataGrid::new("schedule_sequence_list", rect, &tr!("schedule-sequences"))
        .column_header(&tr!("planning-name"))
        .show(ui, |ui| {
            if plan.sequences().is_empty() {
                explorer_note(ui, tr!("schedule-no-sequences"));
            }
            for sequence in plan.sequences() {
                let report = editor.schedule_sequence_reports.iter().find(|report| report.sequence == sequence.id);
                let label = match report.and_then(|report| report.tonnes) {
                    Some(tonnes) => tr_format!(
                        literal = "%name% · %tonnes% %unit%",
                        name = sequence.name.clone(),
                        tonnes = tonnes.to_string(),
                        unit = tr!(literal = "t")
                    ),
                    None => sequence.name.clone(),
                };
                let hover = match report {
                    Some(report) if report.problems.is_empty() && report.tonnes.is_some() => tr!("sequence-ready"),
                    Some(report) if !report.problems.is_empty() => report.problems.join("\n"),
                    _ => sequence.name.clone(),
                };
                let response = grid_row(ui, GridRow::new(&label).selected(selected == Some(sequence.id))).on_hover_text(&hover);
                if response.clicked() {
                    selected = Some(sequence.id);
                }
                context_menu_popup(&response, &sequence.name, |ui| {
                    if ContextMenuAction::new(tr!("schedule-delete-sequence")).show(ui).clicked() {
                        commands.push(UiCommand::schedule(session, ScheduleEdit::DeleteSequence(sequence.id)));
                        ui.close();
                    }
                });
            }
            let body = ui.available_rect_before_wrap();
            if body.is_positive() {
                let response = ui.interact(body, ui.id().with("new_sequence_space"), egui::Sense::click());
                context_menu_popup(&response, tr!("schedule-sequences"), |ui| {
                    if ContextMenuAction::new(tr!("schedule-new-sequence")).show(ui).clicked() {
                        open_dialog = true;
                        ui.close();
                    }
                });
            }
        });
    if editor.schedule_selected_sequence != selected {
        // A different sequence is being edited; the member place selected in
        // the old one belongs to nothing in the new one.
        editor.schedule_sequence_draft = None;
        editor.schedule_selected_member = None;
    }
    editor.schedule_selected_sequence = selected;
    if open_dialog && !editor.new_sequence_open {
        editor.new_sequence_name = suggested_sequence_name(plan);
        editor.new_sequence_open = true;
    }
}

/// The selected sequence's own column: its cells above, every stated problem
/// under them, and the dig order itself below that.
///
/// The dig order is persistent plan data; the names, tonnes and problems
/// beside it are what the current run says about that order, mirrored from
/// the readiness report. A member the run cannot place keeps its place and
/// its red badge - nothing here removes it, renames it, or reads it as zero.
pub(crate) fn draw_sequence_details(ui: &mut egui::Ui, rect: egui::Rect, editor: &mut EditorState, plan: &SchedulePlan, session: u32, commands: &mut Vec<UiCommand>) {
    let Some(sequence) = editor.schedule_selected_sequence.and_then(|id| plan.sequence(id)) else {
        PropertyTable::new("schedule_sequence_properties", rect, &tr!("schedule-sequences")).show(ui, |rows| {
            rows.header(&tr!("planning-property"), &tr!("planning-value"));
            rows.readonly(&tr!("planning-name"), &tr!("schedule-select-sequence"), None, None);
        });
        return;
    };
    let report = editor.schedule_sequence_reports.iter().find(|report| report.sequence == sequence.id).cloned();
    if editor
        .schedule_sequence_draft
        .as_ref()
        .is_none_or(|draft| draft.id != sequence.id || draft.source != sequence.name)
    {
        editor.schedule_sequence_draft = Some(ScheduleSequenceDraft {
            id: sequence.id,
            source: sequence.name.clone(),
            name: sequence.name.clone(),
        });
    }
    let draft = editor.schedule_sequence_draft.as_mut().expect("just ensured");
    let name_error = name_problem(&draft.name, plan.sequences().iter().filter(|other| other.id != sequence.id).map(|other| other.name.clone()));
    let mut edits = Vec::new();
    let properties_rect = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), property_table_height(ui, 3).min(rect.height())));
    PropertyTable::new("schedule_sequence_properties", properties_rect, &sequence.name).show(ui, |rows| {
        rows.header(&tr!("planning-property"), &tr!("planning-value"));
        let name = rows.field(&tr!("planning-name"), &mut draft.name, name_error.as_deref());
        if name.lost_focus()
            && name_problem(&draft.name, plan.sequences().iter().filter(|other| other.id != sequence.id).map(|other| other.name.clone())).is_none()
            && draft.name.trim() != sequence.name
        {
            edits.push(UiCommand::schedule(
                session,
                ScheduleEdit::RenameSequence {
                    sequence: sequence.id,
                    name: draft.name.trim().to_owned(),
                },
            ));
        }
        let tonnes = report
            .as_ref()
            .and_then(|report| report.tonnes)
            .map(|tonnes| tonnes.to_string())
            .unwrap_or_else(|| tr!(literal = "—"));
        rows.readonly(
            &tr!("schedule-sequence-tonnes"),
            &tonnes,
            Some(&tr!(literal = "t")),
            report.as_ref().and_then(|report| report.problems.first()).map(String::as_str),
        );
    });

    // Every problem stated in full, not as a hover: these are the reasons a
    // schedule cannot be calculated, and a truncated badge cannot carry them.
    let problems = report.as_ref().map_or(&[][..], |report| report.problems.as_slice());
    let mut order_top = properties_rect.bottom() + 6.0;
    if !problems.is_empty() {
        let font = egui::TextStyle::Body.resolve(ui.style());
        let color = ui.visuals().error_fg_color;
        let width = rect.width() - 16.0;
        let galleys: Vec<_> = problems.iter().map(|problem| ui.painter().layout(problem.clone(), font.clone(), color, width)).collect();
        let height = galleys.iter().map(|galley| galley.size().y + 4.0).sum::<f32>() + 4.0;
        let problems_rect = egui::Rect::from_min_size(egui::pos2(rect.left() + 8.0, order_top), egui::vec2(rect.width(), height));
        let mut cursor = problems_rect.min;
        for galley in &galleys {
            ui.painter().galley(cursor, galley.clone(), color);
            cursor.y += galley.size().y + 4.0;
        }
        order_top = problems_rect.bottom() + 6.0;
    }

    let order_rect = egui::Rect::from_min_max(egui::pos2(rect.left(), order_top), rect.max);
    if !order_rect.is_positive() {
        commands.extend(edits);
        return;
    }
    let mut selected_member = editor.schedule_selected_member;
    DataGrid::new("schedule_sequence_order", order_rect, &tr!("schedule-sequence-order"))
        .column_header(&tr!("schedule-sequence-blocks"))
        .show(ui, |ui| {
            if sequence.members().is_empty() {
                explorer_note(ui, tr!("schedule-sequence-pick-coming"));
            }
            for (index, _member) in sequence.members().iter().enumerate() {
                let view = report.as_ref().and_then(|report| report.members.get(index));
                let mut label = match view.and_then(|view| view.name.as_deref()) {
                    Some(name) => tr!("schedule-sequence-position", position = (index + 1).to_string(), block = name.to_owned()),
                    None => tr!("schedule-sequence-unresolved-row", position = (index + 1).to_string()),
                };
                if let Some(solid) = view.and_then(|view| view.solid_name.as_deref()) {
                    label = tr_format!(literal = "%label% · %solid%", label = label, solid = solid.to_owned());
                }
                if let Some(tonnes) = view.and_then(|view| view.tonnes) {
                    label = tr_format!(literal = "%label% · %tonnes% %unit%", label = label, tonnes = tonnes.to_string(), unit = tr!(literal = "t"));
                }
                let response = grid_row(
                    ui,
                    GridRow::new(&label)
                        .selected(selected_member == Some(index))
                        .error(view.and_then(|view| view.unresolved.as_deref())),
                )
                .on_hover_text(&label);
                if response.clicked() {
                    selected_member = Some(index);
                }
                context_menu_popup(&response, &label, |ui| {
                    if ContextMenuAction::new(tr!("schedule-sequence-move-up")).enabled(index > 0).show(ui).clicked() {
                        edits.push(UiCommand::schedule(
                            session,
                            ScheduleEdit::MoveMember {
                                sequence: sequence.id,
                                from: index,
                                to: index - 1,
                            },
                        ));
                        ui.close();
                    }
                    if ContextMenuAction::new(tr!("schedule-sequence-move-down"))
                        .enabled(index + 1 < sequence.members().len())
                        .show(ui)
                        .clicked()
                    {
                        edits.push(UiCommand::schedule(
                            session,
                            ScheduleEdit::MoveMember {
                                sequence: sequence.id,
                                from: index,
                                to: index + 1,
                            },
                        ));
                        ui.close();
                    }
                    if ContextMenuAction::new(tr!(literal = "Remove")).show(ui).clicked() {
                        edits.push(UiCommand::schedule(
                            session,
                            ScheduleEdit::RemoveMember {
                                sequence: sequence.id,
                                position: index,
                            },
                        ));
                        ui.close();
                    }
                });
            }
        });
    editor.schedule_selected_member = selected_member;
    commands.extend(edits);
}
