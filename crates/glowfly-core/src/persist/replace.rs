//! The platform steps of a crash-safe write: permissions, the rename over the old file (with
//! Windows' read-only attribute and brief locks), flushing the folder, and old temporary files.

use std::fs::{File, Metadata};
use std::io;
use std::path::Path;
use std::time::Duration;

/// Gives the new file the old one's permissions, with the executable bits as asked (Unix), or
/// the old one's read-only attribute (Windows). Best effort: some file systems (FAT, exFAT,
/// some network shares) refuse every change, and the contents matter more.
pub(super) fn set_mode(f: &File, old: Option<&Metadata>, executable: Option<bool>) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let Ok(created) = f.metadata().map(|m| m.permissions().mode()) else { return };
        let want = super::wanted_mode(old.map(|m| m.permissions().mode()), created, executable);
        if super::needs_chmod(created, want) {
            let _ = f.set_permissions(std::fs::Permissions::from_mode(want));
        }
    }
    #[cfg(not(unix))]
    {
        let _ = executable;
        if old.is_some_and(|m| m.permissions().readonly())
            && let Ok(mut p) = f.metadata().map(|m| m.permissions())
        {
            p.set_readonly(true);
            let _ = f.set_permissions(p);
        }
    }
}

/// Windows errors worth a short wait and another try: access denied (a file being deleted, or
/// held by a scanner), sharing violation and lock violation.
fn retryable(e: &io::Error) -> bool {
    cfg!(windows) && matches!(e.raw_os_error(), Some(5 | 32 | 33))
}

fn pause(attempt: u32) {
    std::thread::sleep(Duration::from_millis(15 * u64::from(attempt)));
}

const ATTEMPTS: u32 = 8;

/// Renames `from` over `to`. `std::fs::rename` replaces an existing target on every platform
/// (on Windows through `MoveFileExW` with `MOVEFILE_REPLACE_EXISTING`). Windows refuses to
/// replace a read-only file, so its attribute is cleared first (the new file carries it, see
/// [`set_mode`]); and it refuses while another program briefly holds the file, which is retried.
pub(super) fn replace(from: &Path, to: &Path, old: Option<&Metadata>) -> io::Result<()> {
    let read_only = cfg!(windows) && old.is_some_and(|m| m.permissions().readonly());
    let mut cleared = false;
    let mut attempt = 0;
    loop {
        match std::fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(e) if read_only && !cleared && e.kind() == io::ErrorKind::PermissionDenied => {
                set_readonly(to, false)?;
                cleared = true;
            }
            Err(e) if retryable(&e) && attempt < ATTEMPTS => {
                attempt += 1;
                pause(attempt);
            }
            Err(e) => {
                if cleared {
                    let _ = set_readonly(to, true);
                }
                return Err(e);
            }
        }
    }
}

/// Removes the file or link at `path` itself. On Windows a read-only file's attribute is
/// cleared first, and a file another program briefly holds is retried.
pub(super) fn remove(path: &Path) -> io::Result<()> {
    let mut cleared = false;
    let mut attempt = 0;
    loop {
        match std::fs::remove_file(path) {
            Err(e)
                if cfg!(windows)
                    && !cleared
                    && e.kind() == io::ErrorKind::PermissionDenied
                    && path.symlink_metadata().is_ok_and(|m| m.permissions().readonly()) =>
            {
                set_readonly(path, false)?;
                cleared = true;
            }
            Err(e) if retryable(&e) && attempt < ATTEMPTS => {
                attempt += 1;
                pause(attempt);
            }
            r => return r,
        }
    }
}

/// Sets or clears the read-only attribute. Only called on Windows, where that is all
/// `set_readonly` changes.
fn set_readonly(path: &Path, on: bool) -> io::Result<()> {
    let mut p = std::fs::symlink_metadata(path)?.permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    p.set_readonly(on);
    std::fs::set_permissions(path, p)
}

/// Flushes the folder on Unix, so a rename in it survives a power cut. Windows (NTFS journals
/// its metadata) has no equivalent for folders. Some file systems refuse; that is ignored.
pub(super) fn sync_dir(dir: &Path) {
    #[cfg(unix)]
    let _ = File::open(dir).and_then(|d| d.sync_all());
    #[cfg(not(unix))]
    let _ = dir;
}

/// How old a temporary file must be before it counts as left behind by a crash. A write takes
/// milliseconds, so a younger one may belong to a write still running in another thread or
/// another Glowfly.
const STALE_AFTER: Duration = Duration::from_secs(10 * 60);

/// Removes temporary files for `name` in `dir` that a crashed write left behind.
pub(super) fn clean_stale_temps(dir: &Path, name: &str) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let file_name = entry.file_name();
        if !super::is_temp_of(name, &file_name.to_string_lossy()) {
            continue;
        }
        let old = entry.metadata().ok().and_then(|m| m.modified().ok()).and_then(|t| t.elapsed().ok()).is_some_and(|age| age > STALE_AFTER);
        if old && entry.file_type().is_ok_and(|t| t.is_file()) {
            let _ = remove(&entry.path());
        }
    }
}
