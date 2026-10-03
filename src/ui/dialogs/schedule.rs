//! The Schedule workspace's New Loader Class and New Loader Agent dialogs,
//! and the Gantt's New Bar / Rename Bar dialog.
//!
//! All of them are drafts: nothing reaches the project until the entry is
//! valid and the user confirms it, so a cancelled dialog leaves the schedule -
//! and the project's dirty marker - exactly as it found them.

use crate::{
    i18n::tr,
    model::{
        Document,
        schedule::{DestinationId, DestinationKind, SchedulePlan, WorkWindow, destinations},
    },
    ui::{
        EditorState,
        state::{BarNameDialog, BarWindowDialog, ScheduleEdit, UiCommand},
        widgets::{
            context_menu::{ChecklistRow, Tick, checklist_popup},
            menu::{self, DragableMenu, MenuButton, MenuField, MenuFieldCombo, MenuFieldText},
        },
    },
};

/// Add a reclaim bar, or edit the stockpile and cumulative cap of an existing
/// one. Creation asks for the loader and complete work window here because it
/// never enters the viewport's dig-block picking mode.
pub(crate) fn draw_reclaim_bar_dialog(ui: &mut egui::Ui, editor: &mut EditorState, plan: &SchedulePlan, document: &Document, session: u32, commands: &mut Vec<UiCommand>) {
    let Some(target) = editor.reclaim_bar_dialog.as_ref().map(|dialog| dialog.target) else {
        return;
    };
    if target.is_some_and(|id| plan.bar(id).is_none_or(|bar| bar.reclaim().is_none())) {
        editor.reclaim_bar_dialog = None;
        return;
    }
    let stockpiles: Vec<_> = destinations::available(document.solids(), plan.routing())
        .into_iter()
        .filter(|entry| entry.kind == DestinationKind::Stockpile)
        .collect();
    let mut open = true;
    let mut close = false;
    let draft = editor.reclaim_bar_dialog.as_mut().expect("checked above");
    let title = if target.is_some() { tr!("reclaim-edit-bar") } else { tr!("reclaim-add-bar") };
    DragableMenu::new("reclaim_bar_dialog", title).open(&mut open).min_width(360.0).show(ui.ctx(), |ui| {
        permitted_stockpiles_field(ui, &mut draft.sources, &stockpiles);

        if target.is_none() {
            let agent_label = draft
                .agent
                .and_then(|id| plan.agent(id))
                .map(|agent| agent.name.clone())
                .unwrap_or_else(|| tr!("reclaim-loader-choose"));
            MenuFieldCombo::new(
                "reclaim_bar_loader",
                tr!("reclaim-loader"),
                &mut draft.agent,
                agent_label,
                plan.agents().iter().map(|agent| (Some(agent.id), agent.name.clone().into())),
            )
            .show(ui);
            MenuFieldText::new(tr!("schedule-window-start"), &mut draft.start).show(ui);
            MenuFieldText::new(tr!("schedule-window-end"), &mut draft.end).show(ui);
        }
        MenuFieldText::new(tr!("reclaim-maximum"), &mut draft.maximum)
            .hint_text(tr!("reclaim-maximum-none"))
            .show(ui);

        let maximum = if draft.maximum.trim().is_empty() {
            Some(None)
        } else {
            draft.maximum.trim().parse::<f64>().ok().filter(|value| value.is_finite() && *value > 0.0).map(Some)
        };
        let window = if target.is_none() {
            let start = draft.start.trim().parse::<f64>().ok();
            let end = draft.end.trim().parse::<f64>().ok();
            start
                .zip(end)
                .map(|(start_h, end_h)| WorkWindow { start_h, end_h: Some(end_h) })
                .filter(|window| window.is_valid())
        } else {
            None
        };
        let valid = !draft.sources.is_empty() && maximum.is_some() && (target.is_some() || (draft.agent.is_some() && window.is_some()));
        if !stockpiles.is_empty() && draft.sources.is_empty() {
            ui.label(egui::RichText::new(tr!("reclaim-source-required")).color(ui.visuals().error_fg_color));
        } else if stockpiles.is_empty() {
            ui.label(egui::RichText::new(tr!("reclaim-no-stockpiles")).color(ui.visuals().error_fg_color));
        }
        if maximum.is_none() {
            ui.label(egui::RichText::new(crate::model::schedule::ScheduleError::InvalidReclaimLimit.message()).color(ui.visuals().error_fg_color));
        }
        menu::menu_actions(ui, |ui| {
            let submitted = menu::dialog_confirm_pressed(ui.ctx());
            if (submitted || ui.add(MenuButton::new(tr!("common-apply")).primary().enabled(valid)).clicked())
                && valid
                && let Some(maximum_t) = maximum
            {
                let sources = draft.sources.clone();
                match target {
                    Some(bar) => {
                        commands.push(UiCommand::schedule(session, ScheduleEdit::SetReclaimSources { bar, sources }));
                        commands.push(UiCommand::schedule(session, ScheduleEdit::SetReclaimMaximum { bar, maximum_t }));
                    }
                    None => commands.push(UiCommand::schedule(
                        session,
                        ScheduleEdit::AddReclaimBar {
                            name: String::new(),
                            agent: draft.agent,
                            priority: draft.priority,
                            window: window.expect("validated above"),
                            sources,
                            maximum_t,
                        },
                    )),
                }
                close = true;
            }
            if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                close = true;
            }
        });
    });
    if close || !open {
        editor.reclaim_bar_dialog = None;
    }
}

/// Add one machine type: a name, and the rate it digs at in tonnes per hour.
pub(crate) fn draw_new_loader_class_dialog(ui: &mut egui::Ui, editor: &mut EditorState, plan: &SchedulePlan, session: u32, commands: &mut Vec<UiCommand>) {
    if !editor.new_loader_class_open {
        return;
    }
    let mut open = true;
    let mut close = false;
    DragableMenu::new("new_loader_class_dialog", tr!("schedule-new-class"))
        .open(&mut open)
        .min_width(320.0)
        .show(ui.ctx(), |ui| {
            MenuFieldText::new(tr!("planning-name"), &mut editor.new_loader_class_name)
                .hint_text(tr!("dialog-rename-field-hint"))
                .show(ui);
            let kind_label = editor.new_loader_class_kind.label();
            MenuFieldCombo::new(
                "new_loader_class_kind",
                tr!("machine-kind"),
                &mut editor.new_loader_class_kind,
                kind_label,
                crate::model::schedule::MachineKind::ALL.map(|kind| (kind, kind.label().into())),
            )
            .show(ui);
            let kind = editor.new_loader_class_kind;
            let (rate_label, unit) = if kind.is_drill_blast() {
                (tr!("machine-work-rate", unit = kind.rate_unit()), kind.rate_unit().to_owned())
            } else {
                (tr!("schedule-dig-rate"), tr!("schedule-tph"))
            };
            MenuFieldText::new(rate_label, &mut editor.new_loader_class_rate).hint_text(unit).show(ui);
            let name = editor.new_loader_class_name.trim().to_owned();
            let rate = editor.new_loader_class_rate.trim().parse::<f64>().ok().filter(|rate| rate.is_finite() && *rate > 0.0);
            // Rejected before it is offered rather than after it is pressed:
            // the same rules the domain enforces, applied to the draft.
            let taken = plan.classes().iter().any(|class| class.name.trim().eq_ignore_ascii_case(&name));
            let can_add = !name.is_empty() && !taken && rate.is_some();
            if taken {
                ui.label(egui::RichText::new(tr!("schedule-error-duplicate-name", name = name.clone())).color(ui.visuals().error_fg_color));
            }
            menu::menu_actions(ui, |ui| {
                let submitted = menu::dialog_confirm_pressed(ui.ctx());
                if (submitted || ui.add(MenuButton::new(tr!("schedule-add-class")).primary().enabled(can_add)).clicked())
                    && let Some(rate_tph) = rate.filter(|_| can_add)
                {
                    commands.push(UiCommand::schedule(session, ScheduleEdit::AddClass { name, rate_tph, kind }));
                    close = true;
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                    close = true;
                }
            });
        });
    if close || !open {
        editor.new_loader_class_open = false;
        editor.new_loader_class_name.clear();
        editor.new_loader_class_rate.clear();
    }
}

/// Add one machine of an existing class. Offered only once a class exists:
/// an agent with no class has no rate, and stage 1 has nothing to fall back
/// on.
pub(crate) fn draw_new_loader_agent_dialog(ui: &mut egui::Ui, editor: &mut EditorState, plan: &SchedulePlan, session: u32, commands: &mut Vec<UiCommand>) {
    if !editor.new_loader_agent_open {
        return;
    }
    if plan.classes().is_empty() {
        editor.new_loader_agent_open = false;
        return;
    }
    if editor.new_loader_agent_class.is_none_or(|id| plan.class(id).is_none()) {
        editor.new_loader_agent_class = plan.classes().first().map(|class| class.id);
    }
    let mut open = true;
    let mut close = false;
    DragableMenu::new("new_loader_agent_dialog", tr!("schedule-new-agent"))
        .open(&mut open)
        .min_width(320.0)
        .show(ui.ctx(), |ui| {
            MenuFieldText::new(tr!("planning-name"), &mut editor.new_loader_agent_name)
                .hint_text(tr!("dialog-rename-field-hint"))
                .show(ui);
            let selected_text = editor
                .new_loader_agent_class
                .and_then(|id| plan.class(id))
                .map(|class| class.name.clone())
                .unwrap_or_default();
            MenuFieldCombo::new(
                "new_loader_agent_class",
                tr!("schedule-class"),
                &mut editor.new_loader_agent_class,
                selected_text,
                plan.classes().iter().map(|class| (Some(class.id), class.name.clone().into())),
            )
            .show(ui);
            if let Some(class) = editor.new_loader_agent_class.and_then(|id| plan.class(id)) {
                let unit = if class.kind.is_drill_blast() {
                    class.kind.rate_unit().to_owned()
                } else {
                    tr!("schedule-tph")
                };
                ui.label(egui::RichText::new(tr!("schedule-effective-rate")).weak());
                ui.label(format!("{} {unit}", class.default_dig_rate_tph));
            }
            let name = editor.new_loader_agent_name.trim().to_owned();
            let taken = plan.agents().iter().any(|agent| agent.name.trim().eq_ignore_ascii_case(&name));
            let can_add = !name.is_empty() && !taken && editor.new_loader_agent_class.is_some();
            if taken {
                ui.label(egui::RichText::new(tr!("schedule-error-duplicate-name", name = name.clone())).color(ui.visuals().error_fg_color));
            }
            menu::menu_actions(ui, |ui| {
                let submitted = menu::dialog_confirm_pressed(ui.ctx());
                if (submitted || ui.add(MenuButton::new(tr!("schedule-add-agent")).primary().enabled(can_add)).clicked())
                    && let Some(class) = editor.new_loader_agent_class.filter(|_| can_add)
                {
                    commands.push(UiCommand::schedule(session, ScheduleEdit::AddAgent { name, class }));
                    close = true;
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                    close = true;
                }
            });
        });
    if close || !open {
        editor.new_loader_agent_open = false;
        editor.new_loader_agent_name.clear();
    }
}

/// Rename one Gantt bar, or clear the name it was given.
///
/// A bar is created without being named - its label comes from the pit, bench
/// and blast its dig order covers - so this only ever overrides that. An empty
/// name is a valid answer and hands the bar back to its ground-derived label,
/// which is why a blank field is not treated as an unfinished one.
pub(crate) fn draw_bar_name_dialog(ui: &mut egui::Ui, editor: &mut EditorState, plan: &SchedulePlan, session: u32, commands: &mut Vec<UiCommand>) {
    let Some(target) = editor.bar_name_dialog.as_ref().map(|dialog: &BarNameDialog| dialog.target) else {
        return;
    };
    // A rename whose bar went away while the dialog was open has nothing left
    // to rename; it closes rather than committing against a missing id.
    if plan.bar(target).is_none() {
        editor.bar_name_dialog = None;
        return;
    }
    let mut open = true;
    let mut close = false;
    let draft = editor.bar_name_dialog.as_mut().expect("checked above");
    DragableMenu::new("bar_name_dialog", tr!("schedule-rename-bar"))
        .open(&mut open)
        .min_width(320.0)
        .show(ui.ctx(), |ui| {
            MenuFieldText::new(tr!("planning-name"), &mut draft.name)
                .hint_text(tr!("schedule-leave-blank-use-ground"))
                .show(ui);
            let name = draft.name.trim().to_owned();
            // Rejected before it is offered rather than after it is pressed:
            // the same rules the domain enforces, applied to the draft. A bar
            // keeping its own name is not a duplicate of itself, and the
            // unnamed bars are not duplicates of each other.
            let taken = !name.is_empty()
                && plan
                    .bars()
                    .iter()
                    .any(|bar| bar.id != target && bar.has_custom_name() && bar.name().trim().eq_ignore_ascii_case(&name));
            let can_commit = !taken;
            if taken {
                ui.label(egui::RichText::new(tr!("schedule-error-duplicate-name", name = name.clone())).color(ui.visuals().error_fg_color));
            }
            menu::menu_actions(ui, |ui| {
                let submitted = menu::dialog_confirm_pressed(ui.ctx());
                if (submitted || ui.add(MenuButton::new(tr!("schedule-rename-bar")).primary().enabled(can_commit)).clicked()) && can_commit {
                    commands.push(UiCommand::schedule(session, ScheduleEdit::RenameBar { bar: target, name }));
                    close = true;
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                    close = true;
                }
            });
        });
    if close || !open {
        editor.bar_name_dialog = None;
    }
}

/// Type a bar's work window exactly.
///
/// The same edit dragging an edge produces, validated by the same rule, so
/// neither route can put a window into the project that the other would
/// refuse. An empty end is open-ended rather than zero - the two are different
/// statements, and only one of them is a mistake.
pub(crate) fn draw_bar_window_dialog(ui: &mut egui::Ui, editor: &mut EditorState, plan: &SchedulePlan, session: u32, commands: &mut Vec<UiCommand>) {
    let Some(bar) = editor.bar_window_dialog.as_ref().map(|dialog: &BarWindowDialog| dialog.bar) else {
        return;
    };
    // A bar that went away while the dialog was open has no window to set; it
    // closes rather than committing against a missing id.
    if plan.bar(bar).is_none() {
        editor.bar_window_dialog = None;
        return;
    }
    let mut open = true;
    let mut close = false;
    let draft = editor.bar_window_dialog.as_mut().expect("checked above");
    DragableMenu::new("bar_window_dialog", tr!("schedule-window-dialog"))
        .open(&mut open)
        .min_width(320.0)
        .show(ui.ctx(), |ui| {
            MenuFieldText::new(tr!("schedule-window-start"), &mut draft.start).show(ui);
            MenuFieldText::new(tr!("schedule-window-end"), &mut draft.end)
                .hint_text(tr!("schedule-window-end-hint"))
                .show(ui);
            let window = draft.window();
            if window.is_none() {
                ui.label(egui::RichText::new(tr!("schedule-window-invalid")).color(ui.visuals().error_fg_color));
            }
            menu::menu_actions(ui, |ui| {
                let submitted = menu::dialog_confirm_pressed(ui.ctx());
                if (submitted || ui.add(MenuButton::new(tr!("schedule-window-apply")).primary().enabled(window.is_some())).clicked())
                    && let Some(window) = window
                {
                    commands.push(UiCommand::schedule(session, ScheduleEdit::SetBarWindow { bar, window }));
                    close = true;
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                    close = true;
                }
            });
        });
    if close || !open {
        editor.bar_window_dialog = None;
    }
}

/// The permitted-stockpile selector of a reclaim bar.
///
/// A checkable popup rather than a combo, because the bar names a *set*. Two
/// things it deliberately does not do: it never offers "All", since the point
/// of the field is that a planner has approved specific piles and a pile
/// created tomorrow is not one of them; and it never drops an entry that no
/// longer resolves, which stays listed, ticked and removable so a deleted
/// stockpile is visible rather than quietly gone.
///
/// Unticking the last entry does nothing: an empty list is not a reclaim bar,
/// and the bar is deleted to say that.
fn permitted_stockpiles_field(ui: &mut egui::Ui, chosen: &mut Vec<DestinationId>, stockpiles: &[destinations::DestinationView]) {
    let summary = match chosen.as_slice() {
        [] => tr!("reclaim-source-choose"),
        [single] => stockpiles
            .iter()
            .find(|entry| entry.id == *single)
            .map(|entry| entry.name.clone())
            .unwrap_or_else(|| tr!("destination-unresolved")),
        several => tr!("reclaim-sources-summary", count = several.len().to_string()),
    };
    let response = MenuField::new(tr!("reclaim-source"))
        .help_text(tr!("reclaim-sources-help"))
        .show(ui, |ui, _, column_width| {
            ui.add_sized([column_width, ui.spacing().interact_size.y], egui::Button::new(summary.clone()).truncate())
        });
    checklist_popup(&response, tr!("reclaim-source"), 260.0, |ui| {
        if stockpiles.is_empty() && chosen.is_empty() {
            menu::menu_note(ui, tr!("reclaim-no-stockpiles"));
        }
        for entry in stockpiles {
            let picked = chosen.contains(&entry.id);
            if ChecklistRow::new(&entry.name, Tick::of(picked, false)).show(ui).toggled {
                toggle_source(chosen, entry.id, picked);
            }
        }
        // Entries the project no longer exposes as stockpiles. Kept visible so
        // a bar pointing at a deleted pile reads as a configuration error the
        // user can repair, rather than as a bar that silently lost a source.
        let unresolved: Vec<DestinationId> = chosen.iter().copied().filter(|id| !stockpiles.iter().any(|entry| entry.id == *id)).collect();
        if !unresolved.is_empty() {
            crate::ui::widgets::context_menu::context_menu_separator(ui);
        }
        for id in unresolved {
            if ChecklistRow::new(&tr!("destination-unresolved"), Tick::On).show(ui).toggled {
                toggle_source(chosen, id, true);
            }
        }
    });
}

/// Tick or untick one permitted stockpile, refusing to empty the list.
fn toggle_source(chosen: &mut Vec<DestinationId>, id: DestinationId, picked: bool) {
    if picked {
        if chosen.len() > 1 {
            chosen.retain(|entry| *entry != id);
        }
    } else {
        chosen.push(id);
    }
}

/// Width of the blast picker's order column beside its 3D view.
const BLAST_LIST_WIDTH: f32 = 360.0;

/// Add a dozer, drill or MPU bar, or change the blasts of one, laid out as the
/// dig sequence editor is: the navigation trees, the run's blasts in 3D, and
/// the bar's order, with the order preview beneath.
pub(crate) fn draw_blast_bar_dialog(
    ui: &mut egui::Ui,
    editor: &mut EditorState,
    project: &crate::ui::UiProjectView,
    document: &Document,
    plan: &SchedulePlan,
    session: u32,
    commands: &mut Vec<UiCommand>,
) {
    let Some(target) = editor.blast_bar_dialog.as_ref().map(|dialog| dialog.target) else {
        return;
    };
    if target.is_some_and(|id| plan.bar(id).is_none_or(|bar| bar.blast_order().is_none())) {
        editor.blast_bar_dialog = None;
        return;
    }
    editor.close_sequence_editor();
    let blasts = editor.schedule_blasts.clone();
    let mut open = true;
    let mut close = false;
    let mut draft = editor.blast_bar_dialog.take().expect("checked above");
    if draft.session != session {
        return;
    }
    let target_changed = target
        .and_then(|id| plan.bar(id))
        .and_then(|bar| bar.blast_order())
        .is_some_and(|order| order.members != draft.opened_from);
    let run_changed = draft.generation.is_some() && draft.generation != editor.blast_sequence_generation;
    let title = if target.is_some() { tr!("blast-edit-title") } else { tr!("blast-add-bar") };
    let screen = ui.ctx().content_rect();
    DragableMenu::new("blast_bar_dialog", title)
        .open(&mut open)
        .min_width(820.0_f32.min(screen.width()))
        .fixed_size(screen.size())
        .pinned(screen.min)
        .show(ui.ctx(), |ui| {
            if target_changed || run_changed {
                ui.colored_label(ui.visuals().error_fg_color, tr!("blast-sequence-changed"));
            }
            if target.is_none() {
                let agent_label = draft
                    .agent
                    .and_then(|id| plan.agent(id))
                    .map(|agent| agent.name.clone())
                    .unwrap_or_else(|| tr!("blast-machine-choose"));
                MenuFieldCombo::new(
                    "blast_bar_machine",
                    tr!("blast-machine"),
                    &mut draft.agent,
                    agent_label,
                    plan.agents()
                        .iter()
                        .filter(|agent| plan.agent_kind(agent.id).is_some_and(crate::model::schedule::MachineKind::is_drill_blast))
                        .map(|agent| (Some(agent.id), agent.name.clone().into())),
                )
                .show(ui);
            }
            if blasts.is_empty() {
                ui.label(egui::RichText::new(tr!("drill-blast-no-blasts")).weak());
            }
            // Sized up front, as the dig sequence editor is: the window is
            // exactly as tall as the screen, so the columns take everything
            // the preview slider and the action row below them do not need.
            let available = ui.available_rect_before_wrap();
            let footer = ui.spacing().interact_size.y * 2.0 + ui.spacing().item_spacing.y * 4.0 + 18.0;
            let body_height = (available.height() - footer).max(160.0);
            let nav_width = super::sequence_editor::NAV_WIDTH.min(available.width() * 0.28);
            let list_width = BLAST_LIST_WIDTH.min(available.width() * 0.32);
            let view_width = (available.width() - nav_width - list_width - ui.spacing().item_spacing.x * 2.0).max(200.0);
            let mut remove = None;
            let mut raise = None;
            let mut lower = None;
            ui.horizontal_top(|ui| {
                super::sequence_editor::draw_navigation_column(ui, editor, project, document, commands, egui::vec2(nav_width, body_height));
                ui.allocate_ui_with_layout(egui::vec2(view_width, body_height), egui::Layout::top_down(egui::Align::Min), |ui| {
                    ui.set_min_size(egui::vec2(view_width, body_height));
                    let height = ui.available_height();
                    draw_blast_sequence_preview(ui, editor, &mut draft, session, height);
                });
                ui.allocate_ui_with_layout(egui::vec2(list_width, body_height), egui::Layout::top_down(egui::Align::Min), |ui| {
                    ui.set_min_size(egui::vec2(list_width, body_height));
                    ui.set_clip_rect(ui.clip_rect().intersect(ui.max_rect()));
                    ui.label(egui::RichText::new(tr!("blast-bar-order")).strong());
                    if draft.members.is_empty() {
                        ui.label(egui::RichText::new(tr!("blast-bar-empty-order")).weak());
                    }
                    let count = draft.members.len();
                    egui::ScrollArea::vertical().id_salt("blast_bar_order").auto_shrink([false, false]).show(ui, |ui| {
                        for (index, member) in draft.members.iter().enumerate() {
                            let label = blasts
                                .iter()
                                .find(|entry| entry.holds(member))
                                .map_or_else(|| tr!("blast-bar-missing"), crate::ui::state::BlastListEntry::label);
                            // Fired at the slider's position, so off the pane:
                            // dimmed here to match.
                            let text = egui::RichText::new(format!("{}. {label}", index + 1));
                            ui.horizontal(|ui| {
                                ui.label(if index < draft.preview { text.weak() } else { text });
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.small_button("✕").clicked() {
                                        remove = Some(index);
                                    }
                                    if ui.add_enabled(index + 1 < count, egui::Button::new("↓").small()).clicked() {
                                        lower = Some(index);
                                    }
                                    if ui.add_enabled(index > 0, egui::Button::new("↑").small()).clicked() {
                                        raise = Some(index);
                                    }
                                });
                            });
                        }
                    });
                });
            });
            if let Some(index) = raise {
                draft.members.swap(index - 1, index);
            }
            if let Some(index) = lower {
                draft.members.swap(index, index + 1);
            }
            if let Some(index) = remove {
                draft.members.remove(index);
                if index < draft.preview {
                    draft.preview -= 1;
                }
            }
            draw_blast_preview_slider(ui, &mut draft);
            let valid = (target.is_some() || draft.agent.is_some()) && !target_changed && !run_changed && !draft.members.is_empty();
            menu::menu_actions(ui, |ui| {
                let submitted = menu::dialog_confirm_pressed(ui.ctx());
                if (submitted || ui.add(MenuButton::new(tr!("common-apply")).primary().enabled(valid)).clicked()) && valid {
                    let members = draft.members.clone();
                    match target {
                        Some(bar) => commands.push(UiCommand::schedule(session, ScheduleEdit::SetBlastMembers { bar, members })),
                        None => commands.push(UiCommand::schedule(
                            session,
                            ScheduleEdit::AddBlastBar {
                                agent: draft.agent,
                                priority: draft.priority,
                                window: WorkWindow {
                                    start_h: draft.start_h,
                                    end_h: Some(draft.end_h),
                                },
                                insert_lane: draft.insert_lane,
                                members,
                            },
                        )),
                    }
                    close = true;
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                    close = true;
                }
            });
        });
    if close || !open {
        editor.blast_bar_dialog = None;
        if matches!(
            editor.solid_preview_pick.map(|pick| pick.owner),
            Some(crate::ui::state::SolidPreviewPickOwner::BlastSequence { .. })
        ) {
            editor.solid_preview_pick = None;
        }
    } else {
        editor.blast_bar_dialog = Some(draft);
    }
}

/// The blast order preview, as the dig sequence editor's: at *k* the first
/// *k* blasts are fired and off the pane, and the next pick lands after them.
fn draw_blast_preview_slider(ui: &mut egui::Ui, draft: &mut crate::ui::state::BlastBarDialog) {
    let total = draft.members.len();
    draft.preview = draft.preview.min(total);
    let before = draft.preview;
    ui.separator();
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(tr!("sequence-editor-preview")).strong());
        let mut position = draft.preview;
        let width = (ui.available_width() - 220.0).max(120.0);
        ui.add_enabled_ui(total > 0, |ui| {
            ui.spacing_mut().slider_width = width;
            ui.add(egui::Slider::new(&mut position, 0..=total).show_value(false));
        });
        draft.preview = position.min(total);
        ui.label(egui::RichText::new(tr!("blast-sequence-preview-at", fired = draft.preview.to_string(), total = total.to_string())).weak());
    });
    if draft.preview != before {
        ui.ctx().request_repaint();
    }
}

pub(crate) fn draw_blast_window_dialog(ui: &mut egui::Ui, editor: &mut EditorState, plan: &SchedulePlan, session: u32, commands: &mut Vec<UiCommand>) {
    use crate::model::schedule::drill_blast::BlastWindow;
    let Some(draft) = editor.blast_window_dialog.as_mut() else { return };
    if draft.session != session {
        editor.blast_window_dialog = None;
        return;
    }
    let mut open = true;
    let mut close = false;
    DragableMenu::new("blast_window_editor", tr!("blast-window-edit"))
        .open(&mut open)
        .min_width(360.0)
        .show(ui.ctx(), |ui| {
            if ui.checkbox(&mut draft.daily, tr!("blast-window-daily")).changed() && draft.daily {
                for value in [&mut draft.start, &mut draft.end] {
                    if let Ok(hour) = value.parse::<f64>() {
                        *value = format!("{}", if hour > 0.0 && hour.rem_euclid(24.0) == 0.0 { 24.0 } else { hour.rem_euclid(24.0) });
                    }
                }
            }
            ui.label(egui::RichText::new(if draft.daily { tr!("blast-window-hour-note") } else { tr!("blast-window-elapsed-note") }).weak());
            MenuFieldText::new(tr!("schedule-window-start"), &mut draft.start).show(ui);
            MenuFieldText::new(tr!("schedule-window-end"), &mut draft.end).show(ui);
            let window = draft
                .start
                .trim()
                .parse::<f64>()
                .ok()
                .zip(draft.end.trim().parse::<f64>().ok())
                .map(|(start_h, end_h)| BlastWindow {
                    id: draft.id.unwrap_or_else(|| draft.opened.iter().map(|window| window.id).max().unwrap_or(0).saturating_add(1)),
                    start_h,
                    end_h,
                    daily: draft.daily,
                })
                .filter(BlastWindow::valid);
            let current = plan.drill_blast().effective_windows() == draft.opened;
            if !current {
                ui.colored_label(ui.visuals().error_fg_color, tr!("blast-window-changed"));
            }
            if window.is_none() {
                ui.colored_label(ui.visuals().error_fg_color, tr!("blast-window-invalid"));
            }
            menu::menu_actions(ui, |ui| {
                let apply = ui.add(MenuButton::new(tr!("common-apply")).primary().enabled(window.is_some() && current)).clicked() || menu::dialog_confirm_pressed(ui.ctx());
                if apply
                    && current
                    && let Some(window) = window
                {
                    let mut windows = draft.opened.clone();
                    if let Some(existing) = windows.iter_mut().find(|entry| entry.id == window.id) {
                        *existing = window;
                    } else {
                        windows.push(window);
                    }
                    commands.push(UiCommand::schedule(session, ScheduleEdit::SetBlastWindows(windows)));
                    close = true;
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                    close = true;
                }
            });
        });
    if close || !open {
        editor.blast_window_dialog = None;
    }
}

fn draw_blast_sequence_preview(ui: &mut egui::Ui, editor: &mut EditorState, draft: &mut crate::ui::state::BlastBarDialog, session: u32, height: f32) {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), height), egui::Sense::click_and_drag());
    let ready = draft.generation.is_some() && draft.generation == editor.blast_sequence_generation;
    if let Some(texture) = editor.solid_preview_texture.filter(|_| ready) {
        ui.painter()
            .image(texture, rect, egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
    } else {
        ui.painter()
            .rect_filled(rect, crate::ui::widgets::toolbar::GROUP_CORNER_RADIUS, crate::ui::widgets::tree_row_colors(ui).1);
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            // A run with nothing left to draw has had everything in view
            // fired by the slider, not no blasts at all.
            if ready && draft.preview > 0 {
                tr!("blast-sequence-all-fired")
            } else {
                tr!("drill-blast-no-blasts")
            },
            egui::TextStyle::Body.resolve(ui.style()),
            ui.visuals().weak_text_color(),
        );
    }
    let scale = ui.ctx().pixels_per_point();
    let size = [(rect.width() * scale).round().max(1.0) as u32, (rect.height() * scale).round().max(1.0) as u32];
    if editor.solid_preview_size_px != size {
        editor.solid_preview_size_px = size;
        ui.ctx().request_repaint();
    }
    let delta = response.drag_delta() * scale;
    if delta != egui::Vec2::ZERO {
        if response.dragged_by(egui::PointerButton::Middle) {
            draft.view.pan_by_pixels([f64::from(delta.x), f64::from(delta.y)], f64::from(size[1]));
        }
        if response.dragged_by(egui::PointerButton::Secondary) {
            draft.view.orbit_by_pixels([f64::from(delta.x), f64::from(delta.y)], f64::from(size[1]));
        }
        ui.ctx().request_repaint();
    }
    if response.hovered() {
        let scroll = ui.input(|input| {
            input
                .events
                .iter()
                .filter_map(|event| match event {
                    egui::Event::MouseWheel { unit, delta, .. } => Some(match unit {
                        egui::MouseWheelUnit::Point => f64::from(delta.y * scale),
                        egui::MouseWheelUnit::Line => f64::from(delta.y) * 100.0,
                        egui::MouseWheelUnit::Page => f64::from(delta.y) * f64::from(size[1]),
                    }),
                    _ => None,
                })
                .sum::<f64>()
        });
        if scroll != 0.0 {
            draft.view.zoom_by_scroll(scroll);
            ui.ctx().request_repaint();
        }
    }
    if ready
        && response.clicked()
        && let Some(pointer) = response.interact_pointer_pos()
    {
        let local = pointer - rect.min;
        editor.solid_preview_pick = Some(crate::ui::state::SolidPreviewPickRequest {
            session,
            owner: crate::ui::state::SolidPreviewPickOwner::BlastSequence { edition: draft.edition },
            generation: draft.generation,
            image: editor.solid_preview_image_revision,
            uv: [local.x / rect.width().max(1.0), local.y / rect.height().max(1.0)],
        });
        ui.ctx().request_repaint();
    }
}
