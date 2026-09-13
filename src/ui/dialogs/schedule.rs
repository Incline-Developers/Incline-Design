//! The Schedule workspace's New Loader Class and New Loader Agent dialogs,
//! and the Gantt's New Bar / Rename Bar dialog.
//!
//! All of them are drafts: nothing reaches the project until the entry is
//! valid and the user confirms it, so a cancelled dialog leaves the schedule -
//! and the project's dirty marker - exactly as it found them.

use crate::{
    i18n::tr,
    model::schedule::SchedulePlan,
    ui::{
        EditorState,
        state::{BarNameDialog, ScheduleEdit, UiCommand},
        widgets::menu::{self, DragableMenu, MenuButton, MenuFieldCombo, MenuFieldText},
    },
};

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
                .hint_text(tr!(literal = "Required"))
                .show(ui);
            MenuFieldText::new(tr!("schedule-dig-rate"), &mut editor.new_loader_class_rate)
                .hint_text(tr!("schedule-tph"))
                .show(ui);
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
                    commands.push(UiCommand::schedule(session, ScheduleEdit::AddClass { name, rate_tph }));
                    close = true;
                }
                if ui.add(MenuButton::new(tr!(literal = "Cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
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
                .hint_text(tr!(literal = "Required"))
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
            if let Some(rate) = editor.new_loader_agent_class.and_then(|id| plan.class(id)).map(|class| class.default_dig_rate_tph) {
                ui.label(egui::RichText::new(tr!("schedule-effective-rate")).weak());
                ui.label(format!("{rate} {}", tr!("schedule-tph")));
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
                if ui.add(MenuButton::new(tr!(literal = "Cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                    close = true;
                }
            });
        });
    if close || !open {
        editor.new_loader_agent_open = false;
        editor.new_loader_agent_name.clear();
    }
}

/// Add one dig sequence. It starts empty: its ground is chosen afterwards,
/// once the 3D block editor exists to pick it from.
/// Name a new Gantt bar, or rename one.
///
/// One dialog for both, because they ask the same question and are subject to
/// the same rule: a bar's name must be its own. Which one it is doing is the
/// dialog's `target` - `None` adds, `Some` renames - and the confirm button
/// says so.
pub(crate) fn draw_bar_name_dialog(ui: &mut egui::Ui, editor: &mut EditorState, plan: &SchedulePlan, session: u32, commands: &mut Vec<UiCommand>) {
    let Some((target, agent, priority, earliest_start_h)) = editor
        .bar_name_dialog
        .as_ref()
        .map(|dialog: &BarNameDialog| (dialog.target, dialog.agent, dialog.priority, dialog.earliest_start_h))
    else {
        return;
    };
    // A rename whose bar went away while the dialog was open has nothing left
    // to rename; it closes rather than committing against a missing id.
    if target.is_some_and(|id| plan.bar(id).is_none()) {
        editor.bar_name_dialog = None;
        return;
    }
    let title = if target.is_some() { tr!("schedule-rename-bar") } else { tr!("schedule-new-bar") };
    let confirm = if target.is_some() { tr!("schedule-rename-bar") } else { tr!("schedule-add-bar") };
    let mut open = true;
    let mut close = false;
    let draft = editor.bar_name_dialog.as_mut().expect("checked above");
    DragableMenu::new("bar_name_dialog", title).open(&mut open).min_width(320.0).show(ui.ctx(), |ui| {
        MenuFieldText::new(tr!("planning-name"), &mut draft.name).hint_text(tr!(literal = "Required")).show(ui);
        // Where the new bar will land, stated rather than assumed: it is
        // created at the instant the lane was right-clicked, which is not
        // necessarily the start of the schedule.
        if target.is_none() {
            ui.label(
                egui::RichText::new(tr!(
                    "schedule-bar-earliest-start",
                    instant = crate::ui::elements::schedule_gantt::instant_label(earliest_start_h * crate::ui::state::GanttView::HOUR)
                ))
                .weak(),
            );
        }
        let name = draft.name.trim().to_owned();
        // Rejected before it is offered rather than after it is pressed: the
        // same rules the domain enforces, applied to the draft. A bar keeping
        // its own name is not a duplicate of itself.
        let taken = plan.bars().iter().any(|bar| Some(bar.id) != target && bar.name().trim().eq_ignore_ascii_case(&name));
        let can_commit = !name.is_empty() && !taken;
        if taken {
            ui.label(egui::RichText::new(tr!("schedule-error-duplicate-name", name = name.clone())).color(ui.visuals().error_fg_color));
        }
        menu::menu_actions(ui, |ui| {
            let submitted = menu::dialog_confirm_pressed(ui.ctx());
            if (submitted || ui.add(MenuButton::new(confirm).primary().enabled(can_commit)).clicked()) && can_commit {
                commands.push(UiCommand::schedule(
                    session,
                    match target {
                        Some(bar) => ScheduleEdit::RenameBar { bar, name },
                        None => ScheduleEdit::AddBar {
                            name,
                            agent,
                            priority,
                            earliest_start_h,
                        },
                    },
                ));
                close = true;
            }
            if ui.add(MenuButton::new(tr!(literal = "Cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                close = true;
            }
        });
    });
    if close || !open {
        editor.bar_name_dialog = None;
    }
}
