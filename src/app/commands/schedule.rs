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
        Command, ReserveFieldId,
        schedule::{BarId, LoaderAgentId, LoaderClassId, SchedulePlan, ScheduleResult},
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
            ScheduleEdit::SetCalendarCells { edits } => self.set_calendar_cells(edits),
            ScheduleEdit::SetTonnageField(field) => self.set_tonnage_field(field),
            ScheduleEdit::SetBarHeight(height) => self.set_bar_height(height),
            ScheduleEdit::AddBar { name, agent, priority, window } => self.add_bar(name, agent, priority, window),
            ScheduleEdit::RenameBar { bar, name } => self.rename_bar(bar, name),
            ScheduleEdit::DeleteBar(bar) => self.delete_bar(bar),
            ScheduleEdit::CopyBar(bar) => self.copy_bar(bar),
            ScheduleEdit::SetBarAgent { bar, agent } => self.set_bar_agent(bar, agent),
            ScheduleEdit::SetBarPriority { bar, priority } => self.set_bar_priority(bar, priority),
            ScheduleEdit::SetBarWindow { bar, window } => self.set_bar_window(bar, window),
            ScheduleEdit::SetBarPlacement {
                bar,
                agent,
                priority,
                window,
                insert_lane,
            } => self.set_bar_placement(bar, agent, priority, window, insert_lane),
            ScheduleEdit::AddMember { bar, position, pick } => self.add_bar_member(bar, position, pick),
            ScheduleEdit::RemoveMember { bar, position } => self.remove_bar_member(bar, position),
            ScheduleEdit::MoveMember { bar, from, to } => self.move_bar_member(bar, from, to),
            ScheduleEdit::SetBarMembers { bar, expected, members } => self.set_bar_members(bar, &expected, &members),
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

    fn set_calendar_cells(&mut self, edits: Vec<crate::model::schedule::CalendarCellEdit>) {
        self.edit_schedule(|plan| plan.set_calendar_cells(&edits));
    }

    /// Nominate the field read as tonnes.
    ///
    /// Deliberately not validated here against the project's Field List: a
    /// field can be deleted afterwards, so the choice has to be checked every
    /// time it is read anyway. Checking it in one place - the readiness
    /// report - is what keeps a stale choice explained rather than silently
    /// dropped. See [`crate::app::commands::schedule_readiness`].
    fn set_tonnage_field(&mut self, field: Option<ReserveFieldId>) {
        self.edit_schedule(|plan| {
            plan.set_tonnage_field(field);
            Ok(())
        });
    }

    fn set_bar_height(&mut self, height: f32) {
        self.edit_schedule(|plan| plan.set_bar_height(height));
    }

    fn add_bar(&mut self, name: String, agent: Option<LoaderAgentId>, priority: u32, window: crate::model::schedule::WorkWindow) {
        let mut added = None;
        self.edit_schedule(|plan| {
            added = Some(plan.add_bar(&name, agent, priority, window)?);
            Ok(())
        });
        if let Some(id) = added {
            self.editor.schedule_selected_bar = Some(id);
            self.editor.schedule_selected_member = None;
        }
    }

    fn rename_bar(&mut self, id: BarId, name: String) {
        self.edit_schedule(|plan| plan.rename_bar(id, &name));
    }

    fn delete_bar(&mut self, id: BarId) {
        self.edit_schedule(|plan| plan.remove_bar(id));
        if self.workspace.active_document().is_none_or(|document| document.schedule().bar(id).is_none()) {
            self.editor.schedule_selected_bar = None;
            self.editor.schedule_selected_member = None;
        }
    }

    /// Copy a bar into an independent one, and select the copy.
    ///
    /// The name is suggested here rather than in the plan, because a suggested
    /// name is a UI convenience: the plan refuses a duplicate outright, and
    /// choosing one for the user is the kind of decision that belongs where
    /// the user can see it. The copy is otherwise a clone - same machine,
    /// same lane, same earliest start and the same ground - which makes it
    /// two bars holding one piece of ground, reported as a conflict rather
    /// than quietly resolved.
    fn copy_bar(&mut self, id: BarId) {
        let Some(plan) = self.workspace.active_document().map(|document| document.schedule()) else {
            return;
        };
        let Some(source) = plan.bar(id) else {
            userspace_warn!("{}", crate::model::schedule::ScheduleError::UnknownBar.message());
            return;
        };
        let name = if source.has_custom_name() {
            let base = tr!("schedule-bar-copy-name", name = source.name().to_owned());
            crate::model::schedule::suggested_name(&base, plan.bars().iter().filter(|bar| bar.has_custom_name()).map(|bar| bar.name().to_owned()))
        } else {
            String::new()
        };
        let mut added = None;
        self.edit_schedule(|plan| {
            added = Some(plan.copy_bar(id, &name)?);
            Ok(())
        });
        if let Some(id) = added {
            self.editor.schedule_selected_bar = Some(id);
            self.editor.schedule_selected_member = None;
        }
    }

    fn set_bar_agent(&mut self, id: BarId, agent: Option<crate::model::schedule::LoaderAgentId>) {
        self.edit_schedule(|plan| plan.set_bar_agent(id, agent));
    }

    fn set_bar_priority(&mut self, id: BarId, priority: u32) {
        self.edit_schedule(|plan| plan.set_bar_priority(id, priority));
    }

    fn set_bar_window(&mut self, id: BarId, window: crate::model::schedule::WorkWindow) {
        self.edit_schedule(|plan| plan.set_bar_window(id, window));
    }

    fn set_bar_placement(&mut self, id: BarId, agent: Option<LoaderAgentId>, priority: u32, window: crate::model::schedule::WorkWindow, insert_lane: bool) {
        self.edit_schedule(|plan| plan.set_bar_placement(id, agent, priority, window, insert_lane));
    }

    fn add_bar_member(&mut self, id: BarId, position: usize, pick: crate::model::schedule::DigBlockPick) {
        // The pick boundary: a reference exists only through a block of the
        // current run, checked here, so nothing downstream ever has to trust
        // a hand-built reference or refresh provenance itself.
        let block = match self.add_dig_block(&pick) {
            Ok(block) => block,
            Err(reason) => {
                userspace_warn!("{}", reason);
                return;
            }
        };
        self.edit_schedule(|plan| plan.insert_bar_member(id, position, block));
    }

    fn remove_bar_member(&mut self, id: BarId, position: usize) {
        self.edit_schedule(|plan| plan.remove_bar_member(id, position));
        // The row under the one that went is the one now at this position;
        // past the end, nothing is selected rather than the wrong thing.
        let length = self
            .workspace
            .active_document()
            .and_then(|document| document.schedule().bar(id))
            .map_or(0, |bar| bar.members().len());
        if self.editor.schedule_selected_member.is_some_and(|selected| selected >= length) {
            self.editor.schedule_selected_member = length.checked_sub(1);
        }
    }

    fn move_bar_member(&mut self, id: BarId, from: usize, to: usize) {
        self.edit_schedule(|plan| plan.move_bar_member(id, from, to));
        if self.editor.schedule_selected_member == Some(from) {
            self.editor.schedule_selected_member = Some(to);
        }
    }

    /// Apply a whole drafted dig order to its bar, as one undo step.
    ///
    /// The sequence editor is a floating window, so it is open across frames
    /// in which anything can happen. Four things are checked here rather than
    /// assumed, and each of them refuses the edit outright: applying half of
    /// a stale draft would be worse than applying none of it.
    ///
    /// - The **project**, by `apply_schedule_edit`'s session token before this
    ///   is reached at all.
    /// - The **bar**, which an undo or a deletion can have taken away.
    /// - The **target order**, against the list the draft was opened from: an
    ///   undo or an edit from elsewhere makes the draft an older version of
    ///   the same order, and writing it back would silently revert the newer
    ///   one.
    /// - Every **new pick**, against the snapshot that is current now - not
    ///   the one it was picked from. A rerun between the pick and Apply
    ///   produces different ground under the same id, and
    ///   [`crate::app::App::add_dig_block`] is the only door a reference
    ///   enters through.
    fn set_bar_members(&mut self, id: BarId, expected: &[crate::model::schedule::DigBlockRef], members: &[crate::ui::state::DraftMember]) {
        let current = self
            .workspace
            .active_document()
            .and_then(|document| document.schedule().bar(id))
            .map(|bar| bar.members().to_vec());
        let resolved = match resolve_drafted_order(current.as_deref(), expected, members, |pick| self.add_dig_block(pick)) {
            Ok(resolved) => resolved,
            Err(refusal) => {
                userspace_warn!("{}", refusal.message());
                // A bar that is gone cannot be edited back into existence, so
                // the window goes with it. Every other refusal leaves the
                // draft on screen, because the draft is where it is fixed.
                if refusal == SequenceRefusal::BarGone {
                    self.editor.close_sequence_editor();
                }
                return;
            }
        };
        let mut committed = false;
        self.edit_schedule(|plan| {
            plan.set_bar_members(id, resolved)?;
            committed = true;
            Ok(())
        });
        // Closed only when the plan accepted the order. A refusal - duplicate
        // ground, say - leaves the window open on the draft that caused it,
        // which is the only place it can be fixed.
        if committed {
            self.editor.close_sequence_editor();
        }
    }
}

/// Why a dig order drafted in the sequence editor was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SequenceRefusal {
    /// The bar is no longer in the schedule - deleted, or undone, while the
    /// window was open.
    BarGone,
    /// The bar's dig order is no longer the one the draft was opened from.
    TargetChanged,
    /// One member picked in the editing session did not survive
    /// revalidation, carrying the pick boundary's own reason.
    Pick(String),
}

impl SequenceRefusal {
    fn message(&self) -> String {
        match self {
            Self::BarGone => crate::model::schedule::ScheduleError::UnknownBar.message(),
            Self::TargetChanged => tr!("sequence-target-changed"),
            Self::Pick(reason) => reason.clone(),
        }
    }
}

/// Turn a drafted member list into the dig order it would replace, or refuse.
///
/// The whole of the decision, kept pure so the three refusals can be checked
/// without a running application: `current` is what the bar holds now
/// (`None` when it is gone), `expected` is what the draft was opened from,
/// and `resolve_pick` is the generation-checked pick boundary -
/// [`crate::app::App::add_dig_block`] in the application, something that
/// refuses on demand in a test.
///
/// Two properties this has to keep. It resolves the *whole* list before
/// returning anything, so a draft carrying one stale pick is refused entire
/// rather than leaving the bar holding the first half of an editing session.
/// And it copies held references through untouched: a member already in the
/// bar keeps the provenance it was captured with, because refreshing a stamp
/// that could no longer be re-derived is exactly the silent repair the
/// identity layer exists to prevent.
fn resolve_drafted_order(
    current: Option<&[crate::model::schedule::DigBlockRef]>,
    expected: &[crate::model::schedule::DigBlockRef],
    members: &[crate::ui::state::DraftMember],
    resolve_pick: impl Fn(&crate::model::schedule::DigBlockPick) -> std::result::Result<crate::model::schedule::DigBlockRef, String>,
) -> std::result::Result<Vec<crate::model::schedule::DigBlockRef>, SequenceRefusal> {
    use crate::ui::state::DraftMember;

    let Some(current) = current else {
        return Err(SequenceRefusal::BarGone);
    };
    if current != expected {
        return Err(SequenceRefusal::TargetChanged);
    }
    let mut resolved = Vec::with_capacity(members.len());
    for member in members {
        match member {
            DraftMember::Held(reference) => resolved.push(*reference),
            DraftMember::Picked(pick) => resolved.push(resolve_pick(pick).map_err(SequenceRefusal::Pick)?),
        }
    }
    Ok(resolved)
}
