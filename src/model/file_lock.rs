//! Advisory locks on project files that several people can reach.
//!
//! A project on a shared drive or a synced folder can be opened by two people
//! at once, and whoever saves last silently throws the other's work away.
//! Opening a project for editing therefore leaves a lock file beside it,
//! `<name>.omf.lock`, naming who holds it. Anyone else who opens the project
//! while that lock is live gets it read-only, told who has it.
//!
//! The lock is a plain file rather than an operating-system lock because OS
//! locks do not survive network shares and sync clients reliably, and give
//! no way to say *who* holds them. The holder rewrites it every
//! [`REFRESH_EVERY`]; one not rewritten for [`STALE_AFTER`] belongs to a
//! session that ended without letting go - a crash, a lost connection - and is
//! taken over by the next person to open the file. A holder that finds its
//! lock rewritten by someone else has lost it, and stops saving to the file.

use std::{
    io::{self, Write},
    path::{Path, PathBuf},
    time::Duration,
};

use serde::{Deserialize, Serialize};

/// How often a held lock is rewritten to show its session is still alive.
pub(crate) const REFRESH_EVERY: Duration = Duration::from_secs(60);

/// How long a lock may go unrefreshed before it is taken to be abandoned.
/// Several refreshes, so a slow share or a sleeping sync client does not
/// hand a live lock to someone else.
pub(crate) const STALE_AFTER: Duration = Duration::from_secs(5 * 60);

/// Who holds a lock, as its file records it.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) struct LockHolder {
    /// The holder's login name.
    pub(crate) user: String,
    /// The holder's computer.
    pub(crate) host: String,
    /// When the holder opened the project, in seconds since the Unix epoch.
    pub(crate) since: i64,
    /// When the holder last rewrote the lock, in the same units.
    pub(crate) refreshed: i64,
    /// Unique to one session's hold, so a holder can tell its own lock from
    /// one taken over by someone else, even someone with the same name on the
    /// same machine.
    pub(crate) token: String,
}

impl LockHolder {
    fn new() -> Self {
        let now = now();
        Self {
            user: user_name(),
            host: host_name(),
            since: now,
            refreshed: now,
            token: uuid::Uuid::new_v4().to_string(),
        }
    }

    /// Whether the lock is old enough to be taken to be abandoned.
    fn is_stale(&self) -> bool {
        now().saturating_sub(self.refreshed) > STALE_AFTER.as_secs() as i64
    }

    /// When the holder opened the project, in local time, for messages.
    pub(crate) fn since_label(&self) -> String {
        chrono::DateTime::from_timestamp(self.since, 0)
            .map(|time| time.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_default()
    }
}

/// A lock this session holds. Dropping it removes the lock file, if it is
/// still this session's.
#[derive(Debug)]
pub(crate) struct ProjectLock {
    path: PathBuf,
    holder: LockHolder,
}

/// What opening a project for editing found.
#[derive(Debug)]
pub(crate) enum Acquired {
    /// The project is this session's to edit.
    Held(ProjectLock),
    /// Someone else is editing it.
    HeldBy(LockHolder),
}

/// What a refresh found.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Refreshed {
    /// Still this session's.
    Held,
    /// Someone else has taken the lock, or it was removed and someone has
    /// taken it since: the project is no longer this session's to save.
    Lost(Option<LockHolder>),
}

/// The lock file for the project at `project`.
pub(crate) fn lock_path(project: &Path) -> PathBuf {
    let mut name = project.file_name().map(|name| name.to_os_string()).unwrap_or_default();
    name.push(".lock");
    project.with_file_name(name)
}

/// Whoever else holds a live lock on `project`, if anyone does.
pub(crate) fn other_holder(project: &Path, mine: Option<&ProjectLock>) -> Option<LockHolder> {
    let holder = read(&lock_path(project)).ok().flatten()?;
    let ours = mine.is_some_and(|lock| lock.holder.token == holder.token);
    (!ours && !holder.is_stale()).then_some(holder)
}

/// Lock `project` for editing by this session, unless someone else has it.
/// A stale or unreadable lock is taken over.
pub(crate) fn acquire(project: &Path) -> io::Result<Acquired> {
    let path = lock_path(project);
    for _ in 0..2 {
        match read(&path) {
            Ok(Some(holder)) if !holder.is_stale() => return Ok(Acquired::HeldBy(holder)),
            Ok(Some(_)) => {
                // Abandoned: take it over below.
                remove_if_present(&path)?;
            }
            Ok(None) => {}
            // Being written right now by someone else, or damaged. A fresh
            // file is the former; an old one is abandoned.
            Err(_) if recently_modified(&path) => return Ok(Acquired::HeldBy(unknown_holder())),
            Err(_) => remove_if_present(&path)?,
        }
        let holder = LockHolder::new();
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                file.write_all(&serde_json::to_vec_pretty(&holder)?)?;
                file.sync_all()?;
                return Ok(Acquired::Held(ProjectLock { path, holder }));
            }
            // Someone else created it between the look and the create:
            // look again, and find them.
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Ok(match read(&path) {
        Ok(Some(holder)) => Acquired::HeldBy(holder),
        _ => Acquired::HeldBy(unknown_holder()),
    })
}

/// Take the lock on `project` whoever holds it. Whoever did loses it, and
/// finds that out at their next refresh.
pub(crate) fn take_over(project: &Path) -> io::Result<ProjectLock> {
    let path = lock_path(project);
    let holder = LockHolder::new();
    write(&path, &holder)?;
    Ok(ProjectLock { path, holder })
}

impl ProjectLock {
    /// The project file this lock is for.
    pub(crate) fn project_path(&self) -> PathBuf {
        let name = self
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_suffix(".lock"))
            .unwrap_or_default();
        self.path.with_file_name(name)
    }

    /// Rewrite the lock to show this session is still alive, unless it is
    /// no longer this session's.
    pub(crate) fn refresh(&mut self) -> io::Result<Refreshed> {
        match read(&self.path) {
            Ok(Some(holder)) if holder.token == self.holder.token => {
                self.holder.refreshed = now();
                write(&self.path, &self.holder)?;
                Ok(Refreshed::Held)
            }
            Ok(Some(holder)) => Ok(Refreshed::Lost(Some(holder))),
            // Removed - by hand, or by a sync client - and nobody has taken it:
            // still ours, so put it back.
            Ok(None) => {
                self.holder.refreshed = now();
                write(&self.path, &self.holder)?;
                Ok(Refreshed::Held)
            }
            // Mid-write by someone else.
            Err(_) => Ok(Refreshed::Lost(None)),
        }
    }

    /// Whether the lock file still names this session, checked just before a
    /// save writes the project.
    pub(crate) fn still_held(&self) -> bool {
        match read(&self.path) {
            Ok(Some(holder)) => holder.token == self.holder.token,
            Ok(None) => true,
            Err(_) => false,
        }
    }
}

impl Drop for ProjectLock {
    fn drop(&mut self) {
        if matches!(read(&self.path), Ok(Some(holder)) if holder.token == self.holder.token) {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

fn read(path: &Path) -> io::Result<Option<LockHolder>> {
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map(Some).map_err(io::Error::other),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// Write the lock whole, through a temporary file renamed over it, so a
/// reader never sees half of one.
fn write(path: &Path, holder: &LockHolder) -> io::Result<()> {
    let mut temporary = path.as_os_str().to_os_string();
    temporary.push(format!(".{}", std::process::id()));
    let temporary = PathBuf::from(temporary);
    std::fs::write(&temporary, serde_json::to_vec_pretty(holder)?)?;
    std::fs::rename(&temporary, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&temporary);
    })
}

fn remove_if_present(path: &Path) -> io::Result<()> {
    match std::fs::remove_file(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

fn recently_modified(path: &Path) -> bool {
    std::fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| modified.elapsed().ok())
        .is_some_and(|age| age < STALE_AFTER)
}

/// A holder whose lock could not be read: someone, somewhere.
fn unknown_holder() -> LockHolder {
    LockHolder {
        user: String::new(),
        host: String::new(),
        since: now(),
        refreshed: now(),
        token: String::new(),
    }
}

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

fn user_name() -> String {
    ["USERNAME", "USER", "LOGNAME"]
        .iter()
        .find_map(|key| std::env::var(key).ok().filter(|value| !value.trim().is_empty()))
        .unwrap_or_default()
}

fn host_name() -> String {
    ["COMPUTERNAME", "HOSTNAME"]
        .iter()
        .find_map(|key| std::env::var(key).ok().filter(|value| !value.trim().is_empty()))
        .or_else(|| {
            std::fs::read_to_string("/etc/hostname")
                .ok()
                .map(|name| name.trim().to_owned())
                .filter(|name| !name.is_empty())
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project_in(dir: &tempfile::TempDir) -> PathBuf {
        let path = dir.path().join("site.omf");
        std::fs::write(&path, b"project").unwrap();
        path
    }

    #[test]
    fn second_session_finds_the_first_holding_it() {
        let dir = tempfile::tempdir().unwrap();
        let project = project_in(&dir);
        let Acquired::Held(first) = acquire(&project).unwrap() else { panic!("free file locks") };
        assert_eq!(lock_path(&project), dir.path().join("site.omf.lock"));
        assert_eq!(first.project_path(), project);
        let Acquired::HeldBy(holder) = acquire(&project).unwrap() else {
            panic!("held file refuses")
        };
        assert_eq!(holder.token, first.holder.token);
        assert!(other_holder(&project, Some(&first)).is_none());
        assert!(other_holder(&project, None).is_some());
        drop(first);
        assert!(!lock_path(&project).exists(), "letting go removes the lock");
        assert!(matches!(acquire(&project).unwrap(), Acquired::Held(_)));
    }

    #[test]
    fn stale_lock_is_taken_over() {
        let dir = tempfile::tempdir().unwrap();
        let project = project_in(&dir);
        let mut abandoned = LockHolder::new();
        abandoned.refreshed -= STALE_AFTER.as_secs() as i64 + 10;
        write(&lock_path(&project), &abandoned).unwrap();
        assert!(other_holder(&project, None).is_none());
        assert!(matches!(acquire(&project).unwrap(), Acquired::Held(_)));
    }

    #[test]
    fn taken_over_lock_is_lost_and_not_removed_by_the_loser() {
        let dir = tempfile::tempdir().unwrap();
        let project = project_in(&dir);
        let Acquired::Held(mut first) = acquire(&project).unwrap() else { panic!() };
        let second = take_over(&project).unwrap();
        assert!(!first.still_held());
        assert_eq!(first.refresh().unwrap(), Refreshed::Lost(Some(second.holder.clone())));
        drop(first);
        assert!(lock_path(&project).exists(), "the loser leaves the new holder's lock alone");
        drop(second);
        assert!(!lock_path(&project).exists());
    }

    #[test]
    fn removed_lock_is_put_back_by_its_holder() {
        let dir = tempfile::tempdir().unwrap();
        let project = project_in(&dir);
        let Acquired::Held(mut lock) = acquire(&project).unwrap() else { panic!() };
        std::fs::remove_file(lock_path(&project)).unwrap();
        assert_eq!(lock.refresh().unwrap(), Refreshed::Held);
        assert!(lock_path(&project).exists());
    }
}
