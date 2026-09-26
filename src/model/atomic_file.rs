use std::{
    ffi::OsString,
    fs::{self, File},
    path::Path,
};

use anyhow::{Context, Result};

/// Write a sibling temporary, then atomically replace the destination.
/// `tempfile` creates the sibling exclusively under a random name, so a
/// predictable temporary symlink is never followed, and the rename stays on
/// the destination filesystem.
pub(crate) fn write_atomic(path: &Path, write: impl FnOnce(&mut File) -> Result<()>) -> Result<()> {
    let parent = path.parent().filter(|parent| !parent.as_os_str().is_empty()).unwrap_or_else(|| Path::new("."));
    let permissions = fs::metadata(path).ok().map(|metadata| metadata.permissions());
    let mut prefix = path.file_name().map_or_else(|| OsString::from("output"), OsString::from);
    prefix.push(".tmp-");

    let mut builder = tempfile::Builder::new();
    builder.prefix(&prefix);
    // `tempfile` defaults to owner-only access; a new file gets what a plain
    // create would, still subject to the umask.
    #[cfg(unix)]
    if permissions.is_none() {
        use std::os::unix::fs::PermissionsExt;
        builder.permissions(fs::Permissions::from_mode(0o666));
    }
    // Dropping the temporary on any error below removes it.
    let mut temporary = builder.tempfile_in(parent).with_context(|| format!("create a temporary beside {}", path.display()))?;

    write(temporary.as_file_mut())?;
    if let Some(permissions) = permissions {
        temporary
            .as_file()
            .set_permissions(permissions)
            .with_context(|| format!("preserve permissions for {}", path.display()))?;
    }
    temporary.as_file().sync_all().with_context(|| format!("sync {}", temporary.path().display()))?;
    temporary
        .persist(path)
        .map_err(|error| error.error)
        .with_context(|| format!("replace {}", path.display()))?;

    // Best-effort directory sync makes the rename durable on filesystems that
    // support syncing directory handles. It is deliberately non-fatal because
    // several supported platforms reject directory syncs.
    if let Ok(directory) = File::open(parent) {
        let _ = directory.sync_all();
    }
    Ok(())
}
