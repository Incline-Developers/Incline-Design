//! The Schedule workspace's setup: the project's loader classes and the
//! machines assigned to them.
//!
//! Unlike the Solids config lists beside it, these edits *are* undoable: one
//! committed cell edit, addition or deletion becomes one
//! [`Command::SetSchedulePlan`] and therefore one Ctrl-Z. The plan is small
//! enough for a snapshot either side, and a snapshot is what guarantees undo
//! puts the same ids back - which every Gantt row is keyed by.
//!
//! Every handler here validates through [`SchedulePlan`] rather than trusting
//! the UI: a command naming a class that has since been deleted, or carrying
//! a rate typed into a stale field, is refused and reported.

use crate::{
    i18n::tr,
    model::{
        Command,
        schedule::{LoaderAgentId, LoaderClassId, SchedulePlan, ScheduleResult},
    },
    ui::state::ScheduleEdit,
    userspace_warn,
};

impl crate::app::App<'_> {
    /// Apply one fleet edit to the project it was drawn from.
    ///
    /// `session` is the runtime id the UI read the plan under. Commands are
    /// queued during a frame and handled after it, so the project can be
    /// closed, reverted or replaced in between - and because a fresh project
    /// numbers its first class `0` too, the ids inside the edit would land on
    /// unrelated equipment rather than failing to resolve. The token is what
    /// makes that case detectable; stage 2's asynchronous picks and
    /// calculations need the same guarantee over much longer gaps.
    pub(crate) fn apply_schedule_edit(&mut self, session: u32, edit: ScheduleEdit) {
        let Some(project) = self.workspace.active_project() else {
            return;
        };
        if project.runtime_id != session {
            userspace_warn!("{}", tr!("schedule-stale-edit"));
            return;
        }
        match edit {
            ScheduleEdit::SetName(name) => self.set_schedule_name(name),
            ScheduleEdit::AddClass { name, rate_tph } => self.add_loader_class(name, rate_tph),
            ScheduleEdit::RenameClass { class, name } => self.rename_loader_class(class, name),
            ScheduleEdit::SetClassRate { class, rate_tph } => self.set_loader_class_rate(class, rate_tph),
            ScheduleEdit::DeleteClass(class) => self.delete_loader_class(class),
            ScheduleEdit::AddAgent { name, class } => self.add_loader_agent(name, class),
            ScheduleEdit::RenameAgent { agent, name } => self.rename_loader_agent(agent, name),
            ScheduleEdit::SetAgentClass { agent, class } => self.set_loader_agent_class(agent, class),
            ScheduleEdit::DeleteAgent(agent) => self.delete_loader_agent(agent),
        }
    }

    /// Apply one edit to the active project's schedule, as a single undo step.
    ///
    /// An edit the plan refuses is reported and dropped; one that changes
    /// nothing - a property field handing back the value it already holds -
    /// is dropped silently, so viewing or re-committing cannot dirty the
    /// project.
    fn edit_schedule(&mut self, edit: impl FnOnce(&mut SchedulePlan) -> ScheduleResult) {
        let Some(document) = self.workspace.active_document() else {
            return;
        };
        let before = document.schedule().clone();
        let mut after = before.clone();
        if let Err(error) = edit(&mut after) {
            userspace_warn!("{}", error.message());
            return;
        }
        if after == before {
            return;
        }
        self.execute_edit(Command::SetSchedulePlan {
            before: Box::new(before),
            after: Box::new(after),
        });
    }

    fn set_schedule_name(&mut self, name: String) {
        self.edit_schedule(|plan| {
            plan.set_name(&name);
            Ok(())
        });
    }

    fn add_loader_class(&mut self, name: String, rate_tph: f64) {
        let mut added = None;
        self.edit_schedule(|plan| {
            added = Some(plan.add_class(&name, rate_tph)?);
            Ok(())
        });
        // Selecting what was just added is editor state, not project state,
        // so it is set here rather than inside the undoable edit.
        if let Some(id) = added {
            self.editor.schedule_selected_class = Some(id);
        }
    }

    fn rename_loader_class(&mut self, id: LoaderClassId, name: String) {
        self.edit_schedule(|plan| plan.rename_class(id, &name));
    }

    fn set_loader_class_rate(&mut self, id: LoaderClassId, rate_tph: f64) {
        self.edit_schedule(|plan| plan.set_class_rate(id, rate_tph));
    }

    fn delete_loader_class(&mut self, id: LoaderClassId) {
        self.edit_schedule(|plan| plan.remove_class(id));
        if self.workspace.active_document().is_none_or(|document| document.schedule().class(id).is_none()) {
            self.editor.schedule_selected_class = None;
            self.editor.schedule_class_draft = None;
        }
    }

    fn add_loader_agent(&mut self, name: String, class_id: LoaderClassId) {
        let mut added = None;
        self.edit_schedule(|plan| {
            added = Some(plan.add_agent(&name, class_id)?);
            Ok(())
        });
        if let Some(id) = added {
            self.editor.schedule_selected_agent = Some(id);
        }
    }

    fn rename_loader_agent(&mut self, id: LoaderAgentId, name: String) {
        self.edit_schedule(|plan| plan.rename_agent(id, &name));
    }

    fn set_loader_agent_class(&mut self, id: LoaderAgentId, class_id: LoaderClassId) {
        self.edit_schedule(|plan| plan.set_agent_class(id, class_id));
    }

    fn delete_loader_agent(&mut self, id: LoaderAgentId) {
        self.edit_schedule(|plan| plan.remove_agent(id));
        if self.workspace.active_document().is_none_or(|document| document.schedule().agent(id).is_none()) {
            self.editor.schedule_selected_agent = None;
            self.editor.schedule_agent_draft = None;
        }
    }
}
