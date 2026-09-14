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
        state::{BarNameDialog, BarWindowDialog, ScheduleEdit, UiCommand},
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
                .hint_text(tr!(literal = "Leave blank to use the ground-derived name"))
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
                if ui.add(MenuButton::new(tr!(literal = "Cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
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
                if ui.add(MenuButton::new(tr!(literal = "Cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                    close = true;
                }
            });
        });
    if close || !open {
        editor.bar_window_dialog = None;
    }
}
