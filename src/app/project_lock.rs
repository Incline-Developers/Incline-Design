//! Read-only projects: an OMF 1 file, or one someone else is editing.
//!
//! A project file opened for editing is locked (see
//! [`crate::model::file_lock`]) for as long as it stays open. A project that
//! cannot be saved to its own file can still be edited; saving it asks, in
//! the read-only save dialog, for another file to save to - or, when someone
//! else holds the lock, whether to take it over.

#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;

use crate::{app::App, i18n::tr, userspace_warn};
#[cfg(not(target_arch = "wasm32"))]
use crate::{
    model::{
        file_lock::{self, Acquired, LockHolder, Refreshed},
        project::ReadOnlyReason,
    },
    userspace_log,
};

/// The project's file name, for messages.
#[cfg(not(target_arch = "wasm32"))]
fn file_label(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| path.display().to_string())
}

/// Who holds a lock, for messages.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn holder_label(holder: &LockHolder) -> String {
    if holder.user.is_empty() && holder.host.is_empty() {
        tr!("lock-holder-unknown")
    } else {
        tr!("lock-holder", user = holder.user.clone(), host = holder.host.clone())
    }
}

impl<'a> App<'a> {
    /// Lock a project just opened from `path`, or open it read-only when
    /// someone else has it.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn lock_opened_project(&mut self, path: &Path) {
        self.project_lock = None;
        match file_lock::acquire(path) {
            Ok(Acquired::Held(lock)) => {
                self.project_lock = Some(lock);
                self.project_lock_refreshed_at = Some(web_time::Instant::now());
            }
            Ok(Acquired::HeldBy(holder)) => {
                userspace_warn!(
                    "{}",
                    tr!(
                        "lock-opened-read-only",
                        name = file_label(path),
                        holder = holder_label(&holder),
                        since = holder.since_label()
                    )
                );
                if let Some(project) = self.workspace.active_project_mut() {
                    project.read_only = Some(ReadOnlyReason::LockedBy(holder));
                }
            }
            // A share that refuses new files still lets the project be
            // edited and saved; only the warning to others is missing.
            Err(error) => userspace_warn!("{}", tr!("lock-failed", name = file_label(path), error = error.to_string())),
        }
    }

    /// Lock the file a project was just saved as, which is its file now.
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn lock_saved_project(&mut self, path: &Path) {
        if self.project_lock.as_ref().is_some_and(|lock| lock.project_path() == path && lock.still_held()) {
            return;
        }
        self.project_lock = None;
        match file_lock::acquire(path) {
            Ok(Acquired::Held(lock)) => {
                self.project_lock = Some(lock);
                self.project_lock_refreshed_at = Some(web_time::Instant::now());
            }
            // Taken between the check before the save and now: the save
            // went through, but the file is theirs from here on.
            Ok(Acquired::HeldBy(holder)) => self.lose_project_lock(Some(holder)),
            Err(error) => userspace_warn!("{}", tr!("lock-failed", name = file_label(path), error = error.to_string())),
        }
    }

    /// Refuse a Save As onto a file someone else is editing.
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn ensure_save_as_target_free(&self, path: &Path) -> anyhow::Result<()> {
        match file_lock::other_holder(path, self.project_lock.as_ref()) {
            Some(holder) => anyhow::bail!(tr!("lock-save-as-held", name = file_label(path), holder = holder_label(&holder))),
            None => Ok(()),
        }
    }

    /// Let go of the open project's lock, for a project being closed or
    /// replaced.
    pub(super) fn release_project_lock(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.project_lock = None;
            self.project_lock_refreshed_at = None;
        }
    }

    /// Keep the open project's lock alive, and notice when it has been taken.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn refresh_project_lock(&mut self) {
        let now = web_time::Instant::now();
        if self.project_lock_refreshed_at.is_some_and(|at| now.duration_since(at) < file_lock::REFRESH_EVERY) {
            return;
        }
        self.project_lock_refreshed_at = Some(now);
        let Some(lock) = self.project_lock.as_mut() else { return };
        match lock.refresh() {
            Ok(Refreshed::Held) => {}
            Ok(Refreshed::Lost(holder)) => self.lose_project_lock(holder),
            // A share that has gone away for a moment: try again next time.
            Err(error) => log::warn!("Could not refresh the project lock: {error}"),
        }
    }

    /// The lock was taken by someone else: the project is theirs to save.
    #[cfg(not(target_arch = "wasm32"))]
    fn lose_project_lock(&mut self, holder: Option<LockHolder>) {
        let Some(path) = self
            .project_lock
            .take()
            .map(|lock| lock.project_path())
            .or_else(|| self.workspace.active_project().and_then(|project| project.path.clone()))
        else {
            return;
        };
        let holder = holder.unwrap_or_else(|| {
            file_lock::other_holder(&path, None).unwrap_or(LockHolder {
                user: String::new(),
                host: String::new(),
                since: 0,
                refreshed: 0,
                token: String::new(),
            })
        });
        userspace_warn!("{}", tr!("lock-lost", name = file_label(&path), holder = holder_label(&holder)));
        if let Some(project) = self.workspace.active_project_mut() {
            project.read_only = Some(ReadOnlyReason::LockedBy(holder));
        }
        self.redraw_requested = true;
    }

    /// Before a save writes the project to its own file: whether it may.
    /// When it may not, the read-only save dialog is opened instead.
    pub(super) fn may_save_in_place(&mut self, runtime_id: u32) -> bool {
        let Some(index) = self.workspace.project_index_for_runtime_id(runtime_id) else {
            return false;
        };
        #[cfg(not(target_arch = "wasm32"))]
        if self.workspace.projects[index].read_only.is_none()
            && let Some(lock) = self.project_lock.as_ref()
            && !lock.still_held()
        {
            let holder = file_lock::other_holder(&lock.project_path(), None);
            self.lose_project_lock(holder);
        }
        if self.workspace.projects[index].read_only.is_none() {
            return true;
        }
        self.editor.read_only_save_prompt = Some(runtime_id);
        self.redraw_requested = true;
        false
    }

    /// The read-only save dialog's Save As.
    pub(crate) fn read_only_save_as(&mut self) {
        let Some(runtime_id) = self.editor.read_only_save_prompt.take() else { return };
        #[cfg(not(target_arch = "wasm32"))]
        self.spawn_save_project_as_dialog(runtime_id);
        #[cfg(target_arch = "wasm32")]
        {
            // The browser has no file to be read-only about: an OMF 1 project
            // saves to a new browser record, under its name.
            if let Some(project) = self.workspace.project_index_for_runtime_id(runtime_id).map(|index| &mut self.workspace.projects[index]) {
                project.read_only = None;
            }
            if let Err(error) = self.save_project(runtime_id) {
                userspace_warn!("{}", tr!("cmd-file-save-failed-message", message = format!("{error:#}")));
            }
        }
    }

    /// The read-only save dialog's Take Over and Save: claim the lock from
    /// whoever holds it, and save to the project's own file.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn take_over_and_save(&mut self) -> anyhow::Result<()> {
        let Some(runtime_id) = self.editor.read_only_save_prompt.take() else {
            return Ok(());
        };
        {
            let Some(index) = self.workspace.project_index_for_runtime_id(runtime_id) else {
                return Ok(());
            };
            let Some(ReadOnlyReason::LockedBy(holder)) = self.workspace.projects[index].read_only.clone() else {
                return Ok(());
            };
            let Some(path) = self.workspace.projects[index].path.clone() else {
                return Ok(());
            };
            let lock = file_lock::take_over(&path).map_err(|error| anyhow::anyhow!(tr!("lock-failed", name = file_label(&path), error = error.to_string())))?;
            self.project_lock = Some(lock);
            self.project_lock_refreshed_at = Some(web_time::Instant::now());
            self.workspace.projects[index].read_only = None;
            userspace_log!("{}", tr!("lock-taken-over", name = file_label(&path), holder = holder_label(&holder)));
            self.save_project(runtime_id)
        }
    }

    /// The read-only save dialog's Cancel: give up the save, and whatever was
    /// waiting on it.
    pub(crate) fn cancel_read_only_save(&mut self) {
        self.editor.read_only_save_prompt = None;
        if self.project_replacement_after_save {
            self.project_replacement_after_save = false;
            self.editor.replace_project_confirm_open = true;
        }
        if self.exit_after_pending_saves {
            self.cancel_exit_request();
        }
    }
}
