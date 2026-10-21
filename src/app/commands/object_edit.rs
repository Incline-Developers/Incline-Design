//! "Edit Object" dialog plumbing: seed its working copy, keep it pointed at a
//! live object, and write that copy back as one undoable replace.
//!
//! Every document lookup here goes through the active project, never the
//! composite `scene_document`: it omits hidden objects, and ids restart per
//! project, so it could resolve another project's object of the same id.

use crate::{
    app::App,
    i18n::tr,
    model::{
        Command, Object, ObjectId,
        object_edit::{drift_detected, reverse_vertices, validate_object},
    },
    userspace_log, userspace_warn,
};

impl<'a> App<'a> {
    /// Reverse every selected string as one undoable batch, the way the
    /// dialog's Reverse does one; hidden and locked strings are left alone.
    pub(crate) fn reverse_selected_strings(&mut self) {
        let selected = self.selected_polylines();
        let Some(document) = self.workspace.active_document() else {
            return;
        };
        let replacements: Vec<Command> = selected
            .into_iter()
            .filter(|&id| self.editor.canvas_edits_object(document, id))
            .filter_map(|id| {
                let before = document.get_object(id)?.clone();
                let mut after = before.clone();
                let Object::Polyline { verts, closed, .. } = &mut after else {
                    return None;
                };
                reverse_vertices(verts, *closed);
                (before != after).then_some(Command::Replace { before, after })
            })
            .collect();
        if replacements.is_empty() {
            userspace_warn!("{}", tr!("cmd-object-edit-no-strings-reverse"));
            return;
        }
        let count = replacements.len();
        self.execute_edit(Command::Batch(replacements));
        userspace_log!("{}", tr!("cmd-object-edit-reversed-strings", count = count.to_string()));
        self.invalidate_geometry();
    }

    /// Open the "Edit Object" dialog on `id`, seeding its working copy from
    /// the active project's document (not `scene_document`, the read-only
    /// composite: the dialog needs the editable copy).
    pub(crate) fn open_object_edit_dialog(&mut self, id: ObjectId) {
        // Already open on this object: leave the in-progress working copy
        // alone rather than clobbering it with a fresh seed.
        if self.editor.object_edit_dialog.as_ref().is_some_and(|dialog| dialog.id == id) {
            return;
        }
        let Some(document) = self.workspace.active_document() else {
            userspace_warn!("{}", tr!("cmd-object-edit-select-single-design-object-edit"));
            return;
        };
        let Some(object) = document.get_object(id).cloned() else {
            userspace_warn!("{}", tr!("cmd-object-edit-select-single-design-object-edit"));
            return;
        };
        // The layer's name and colour are read once here and carried on the
        // dialog; the dialog cannot change an object's layer.
        let layer = document.layer(object.layer());
        let layer_name = layer.map(|layer| layer.name.clone()).unwrap_or_else(|| tr!("cmd-object-edit-unassigned"));
        let layer_rgba = layer.map_or([1.0, 1.0, 1.0, 1.0], |layer| layer.color);
        self.editor.object_edit_dialog = Some(crate::ui::dialogs::object_edit::ObjectEditDialog::new(id, object, layer_name, layer_rgba));
    }

    /// Open the "Edit Object" dialog on `id` at its Vertices tab with `row`,
    /// counting from zero, selected and scrolled into view once; a dialog
    /// already open on `id` keeps its working copy and moves to the row.
    pub(crate) fn show_object_vertex(&mut self, id: ObjectId, row: usize) {
        self.activate_project_for_object(id);
        self.open_object_edit_dialog(id);
        if let Some(dialog) = self.editor.object_edit_dialog.as_mut().filter(|dialog| dialog.id == id) {
            dialog.tab = crate::ui::dialogs::object_edit::ObjectEditTab::Vertices;
            dialog.selected_row = Some(row);
            dialog.scroll_to_row = Some(row);
        }
    }

    /// Close the "Edit Object" dialog if its object is gone from the active
    /// project, once per frame before the UI runs. Hiding an object must NOT
    /// close it: an object can still be edited while hidden, and only the
    /// active document says whether it still exists.
    pub(crate) fn refresh_object_edit_dialog(&mut self) {
        let Some(id) = self.editor.object_edit_dialog.as_ref().map(|dialog| dialog.id) else {
            return;
        };
        if self.workspace.active_document().is_some_and(|document| document.get_object(id).is_some()) {
            return;
        }
        userspace_warn!("{}", tr!("cmd-object-edit-object-no-longer-exists-document"));
        self.editor.object_edit_dialog = None;
    }

    /// Write the "Edit Object" dialog's working copy back to the document as
    /// one undoable [`Command::Replace`]. `close` shuts the dialog once the
    /// write went through or there was nothing to write; a refusal keeps it open.
    pub(crate) fn apply_object_edit(&mut self, id: ObjectId, object: Object, close: bool) {
        if object.id() != id {
            self.editor.object_edit_dialog = None;
            userspace_warn!("{}", tr!("cmd-object-edit-target-changed"));
            return;
        }
        let Some(before) = self.workspace.active_document().and_then(|document| document.get_object(id)).cloned() else {
            self.editor.object_edit_dialog = None;
            userspace_warn!("{}", tr!("cmd-object-edit-object-no-longer-exists-document"));
            return;
        };
        // Something else changed this object since the dialog opened. Writing
        // the working copy back now would silently revert that edit; refuse instead.
        if self.editor.object_edit_dialog.as_ref().is_some_and(|dialog| drift_detected(&dialog.baseline, &before)) {
            userspace_warn!("{}", tr!("cmd-object-edit-object-changed-since-editor-opened"));
            // Closed here so that Edit reopens on the current version instead of the stale copy.
            self.editor.object_edit_dialog = None;
            return;
        }
        if let Some(issue) = validate_object(&object) {
            // Same wording the dialog shows inline, so a refusal reads the same in both places.
            userspace_warn!("{}", crate::ui::dialogs::object_edit::issue_message(issue));
            return;
        }
        if before == object {
            // Nothing changed: no history entry. Say so inline rather than
            // leaving Apply and a no-op indistinguishable.
            if close {
                self.editor.object_edit_dialog = None;
            } else if let Some(dialog) = &mut self.editor.object_edit_dialog {
                dialog.message = Some(tr!("cmd-object-edit-no-changes-apply"));
            }
            return;
        }
        self.execute_edit(Command::Replace { before, after: object.clone() });
        self.invalidate_geometry();
        // Re-seat the baseline from the document's copy, not the copy just
        // handed in: a later Apply diffs against what was written, and a
        // replace is free to leave the stored object carrying more than the
        // dialog sent. No `Object` field is outside the dialog's reach today,
        // so the working copy itself needs no re-seating.
        let stored = self.workspace.active_document().and_then(|document| document.get_object(id)).cloned();
        if let Some(dialog) = &mut self.editor.object_edit_dialog {
            dialog.baseline = stored.unwrap_or_else(|| object.clone());
        }
        let vertex_count = match &object {
            Object::Polyline { verts, .. } => Some(verts.len()),
            Object::Point { .. } | Object::Circle { .. } | Object::Text { .. } => None,
        };
        match vertex_count {
            Some(count) => userspace_log!(
                "{}",
                tr!(
                    "cmd-object-edit-edited-kind-count-vertices",
                    kind = object.kind_name().to_string(),
                    count = count.to_string()
                )
            ),
            None => userspace_log!("{}", tr!("cmd-object-edit-edited-kind", kind = object.kind_name().to_string())),
        }
        if close {
            self.editor.object_edit_dialog = None;
        }
    }
}
